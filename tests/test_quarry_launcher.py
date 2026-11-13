"""Exercise PostgreSQL startup and Helix guards using isolated command stubs."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
CONTAINER = "quarry-launcher-test"
VOLUME = f"{CONTAINER}-data"

DOCKER_STUB = r'''#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
folder = pathlib.Path(os.environ["QUARRY_TEST_FOLDER"])
with (folder / "commands.jsonl").open("a") as log:
    log.write(json.dumps(args) + "\n")
scenario = os.environ["QUARRY_TEST_SCENARIO"]
if args[:2] == ["image", "inspect"]:
    print("pinned-image")
elif args[:2] == ["container", "inspect"]:
    if scenario == "absent" and not (folder / "created").exists():
        sys.exit(1)
    if "--format" in args:
        template = args[args.index("--format") + 1]
        if template == "{{.Image}}":
            print("old-image" if scenario == "wrong-image" else "pinned-image")
        elif template == "{{.Config.Image}}":
            print("old-image")
        elif ".Config.Env" in template:
            if scenario == "inspect-error":
                sys.exit(1)
            print("" if scenario == "memory" else "true")
        elif ".Mounts" in template:
            print({"wrong-volume": "other-data", "unmounted": "", "read-only": ""}
                  .get(scenario, os.environ["QUARRY_HELIX_CONTAINER_NAME"] + "-data"))
        elif ".HostConfig.PortBindings" in template:
            print("127.0.0.1:" + os.environ["QUARRY_HELIX_PORT"])
        elif template == "{{.State.Running}}":
            print("false")
        else:
            sys.exit("unexpected inspect template")
elif args[0] == "create":
    (folder / "created").touch()
elif args[0] == "start":
    # End the test after storage validation, before any product process starts.
    sys.exit(41)
elif args != ["info"]:
    sys.exit("unexpected Docker operation")
'''


class LauncherTests(unittest.TestCase):
    def run_launcher(
        self, scenario, postgres_status=0, start_status=0,
        target="web", database_status=0,
    ):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            docker = folder / "docker"
            docker.write_text(DOCKER_STUB)
            docker.chmod(0o755)
            for name in ("cargo", "npm", "curl", "node"):
                stub = folder / name
                stub.write_text("#!/bin/sh\nexit 0\n")
                stub.chmod(0o755)
            postgres = folder / "pg_ctl"
            postgres.write_text("""#!/usr/bin/env python3
import json, os, pathlib, sys
folder = pathlib.Path(os.environ["QUARRY_TEST_FOLDER"])
with (folder / "postgres.jsonl").open("a") as log:
    log.write(json.dumps(sys.argv[1:]) + "\\n")
if sys.argv[-1] == "status":
    sys.exit(int(os.environ["QUARRY_TEST_POSTGRES_STATUS"]))
sys.exit(int(os.environ["QUARRY_TEST_START_STATUS"]))
""")
            postgres.chmod(0o755)
            (folder / "uname").write_text("#!/bin/sh\necho Darwin\n")
            (folder / "uname").chmod(0o755)
            (folder / "cargo").write_text(
                "#!/bin/sh\nexit " + str(database_status) + "\n")
            result = subprocess.run(
                ["bash", str(ROOT / "quarry"), target],
                cwd=ROOT,
                env={
                    **os.environ,
                    "PATH": f"{folder}:{os.environ['PATH']}",
                    "QUARRY_HELIX_CONTAINER_NAME": CONTAINER,
                    "QUARRY_HELIX_PORT": "6970",
                    "QUARRY_POSTGRES_BIN": str(folder),
                    "QUARRY_POSTGRES_DATA_DIR": str(folder),
                    "QUARRY_TEST_POSTGRES_STATUS": str(postgres_status),
                    "QUARRY_TEST_START_STATUS": str(start_status),
                    "QUARRY_TEST_FOLDER": str(folder),
                    "QUARRY_TEST_SCENARIO": scenario,
                },
                capture_output=True,
                text=True,
                timeout=10,
            )
            docker_log = folder / "commands.jsonl"
            commands = (
                [json.loads(line) for line in docker_log.read_text().splitlines()]
                if docker_log.exists() else []
            )
            postgres_commands = [json.loads(line) for line in
                                 (folder / "postgres.jsonl").read_text().splitlines()]
            self.assertEqual(any("start" in c for c in postgres_commands), postgres_status == 3)
            self.assertFalse(any("initdb" in c or "stop" in c for c in postgres_commands))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertFalse(any(c[0] in ("rm", "stop", "rename") for c in commands))
        return result, commands

    def test_stopped_postgres_starts_for_both_targets(self):
        for target in ("web", "desktop"):
            with self.subTest(target=target):
                result, commands = self.run_launcher("disk", postgres_status=3, target=target)
                self.assertIn("Starting local PostgreSQL", result.stdout)
                self.assertIn(["start", CONTAINER], commands)

    def test_postgres_start_failure_stops_before_helix(self):
        result, commands = self.run_launcher("disk", postgres_status=3, start_status=1)
        self.assertIn("could not start the existing PostgreSQL", result.stderr)
        self.assertEqual(commands, [])

    def test_uninspectable_postgres_is_not_started(self):
        result, commands = self.run_launcher("disk", postgres_status=4)
        self.assertIn("could not inspect the existing PostgreSQL", result.stderr)
        self.assertEqual(commands, [])

    def test_database_connection_failure_stops_before_helix(self):
        result, commands = self.run_launcher("disk", database_status=1)
        self.assertIn("PostgreSQL is unavailable", result.stderr)
        self.assertEqual(commands, [])

    def test_new_container_has_disk_storage_and_named_volume(self):
        result, commands = self.run_launcher("absent")
        create = next(c for c in commands if c[0] == "create")
        self.assertIn("HELIX_DATA_DIR=/var/lib/helix", create)
        self.assertIn(f"type=volume,source={VOLUME},target=/var/lib/helix", create)
        self.assertIn("127.0.0.1:6970:8080", create)
        self.assertIn("could not start Helix", result.stderr)
        self.assertIn(["start", CONTAINER], commands)

    def test_existing_disk_container_is_reused(self):
        result, commands = self.run_launcher("disk")
        self.assertFalse(any(c[0] == "create" for c in commands))
        self.assertIn(["start", CONTAINER], commands)
        self.assertIn("could not start Helix", result.stderr)

    def test_unsafe_storage_is_rejected_before_start(self):
        for scenario in ("memory", "wrong-volume", "unmounted", "read-only"):
            with self.subTest(scenario=scenario):
                result, commands = self.run_launcher(scenario)
                self.assertIn("must use HELIX_DATA_DIR", result.stderr)
                self.assertFalse(any(c[0] in ("create", "start") for c in commands))

    def test_storage_inspection_failure_is_rejected(self):
        result, commands = self.run_launcher("inspect-error")
        self.assertIn("could not inspect Helix storage", result.stderr)
        self.assertFalse(any(c[0] in ("create", "start") for c in commands))

    def test_image_mismatch_still_preserves_container(self):
        result, commands = self.run_launcher("wrong-image")
        self.assertIn("not pinned", result.stderr)
        self.assertFalse(any(c[0] in ("create", "start") for c in commands))


if __name__ == "__main__":
    unittest.main()
