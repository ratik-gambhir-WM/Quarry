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
                  .get(scenario, os.environ.get("QUARRY_HELIX_CONTAINER_NAME", "helix-quarry-v3-dev") + "-data"))
        elif ".HostConfig.PortBindings" in template:
            print("127.0.0.1:" + os.environ.get("QUARRY_HELIX_PORT", "6969"))
        elif template == "{{.State.Running}}":
            print("false")
        else:
            sys.exit("unexpected inspect template")
elif args[0] == "create":
    (folder / "created").touch()
elif args[0] == "start":
    # Only the backend-failure scenario proceeds to application startup.
    sys.exit(0 if scenario == "backend-failure" else 41)
elif args != ["info"]:
    sys.exit("unexpected Docker operation")
'''


class LauncherTests(unittest.TestCase):
    def run_launcher(
        self, scenario, postgres_status=0, start_status=0,
        target="web", backend_status=17, use_defaults=False, config=None,
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
                '#!/bin/sh\nprintf "%s\\n" "$*" >> "$QUARRY_TEST_FOLDER/cargo-commands"\n'
                'exit ' + str(backend_status) + "\n")
            (folder / "npm").write_text(
                '#!/bin/sh\ntouch "$QUARRY_TEST_FOLDER/ui-started"\nexit 0\n')
            (folder / "curl").write_text(
                '#!/bin/sh\ncase "$*" in *healthz*) exit 0;; *) exit 1;; esac\n')
            launcher_root = folder / "repo"
            (launcher_root / "backend").mkdir(parents=True)
            (launcher_root / "frontend").mkdir()
            (launcher_root / "quarry").write_text((ROOT / "quarry").read_text())
            (launcher_root / "backend/helix.toml").write_text(
                (ROOT / "backend/helix.toml").read_text() if config is None else config)
            inherited_env = dict(os.environ)
            for key in ("QUARRY_HELIX_CONTAINER_NAME", "QUARRY_HELIX_PORT"):
                inherited_env.pop(key, None)
            selection = {} if use_defaults else {
                "QUARRY_HELIX_CONTAINER_NAME": CONTAINER,
                "QUARRY_HELIX_PORT": "6970",
            }
            result = subprocess.run(
                ["bash", str(launcher_root / "quarry"), target],
                cwd=launcher_root,
                env={
                    **inherited_env,
                    **selection,
                    "PATH": f"{folder}:{os.environ['PATH']}",
                    "QUARRY_POSTGRES_BIN": str(folder),
                    "QUARRY_POSTGRES_DATA_DIR": str(folder),
                    "QUARRY_TEST_POSTGRES_STATUS": str(postgres_status),
                    "QUARRY_TEST_START_STATUS": str(start_status),
                    "QUARRY_TEST_FOLDER": str(folder),
                    "QUARRY_TEST_SCENARIO": scenario,
                },
                capture_output=True,
                text=True,
                timeout=30,
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
            cargo_log = folder / "cargo-commands"
            cargo_commands = cargo_log.read_text().splitlines() if cargo_log.exists() else []
            self.assertEqual(cargo_commands, ["run --locked"] if scenario == "backend-failure" else [])
            self.assertFalse((folder / "ui-started").exists())
        expected_status = (backend_status or 1) if scenario == "backend-failure" else 1
        self.assertEqual(result.returncode, expected_status, result.stdout + result.stderr)
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

    def test_backend_connection_failure_exits_without_preflight_or_ui(self):
        for target in ("web", "desktop"):
            for status in (0, 17):
                with self.subTest(target=target, status=status):
                    result, commands = self.run_launcher(
                        "backend-failure", backend_status=status, target=target)
                    self.assertIn(["start", CONTAINER], commands)
                    self.assertIn("Axum API exited before becoming ready", result.stderr)
                    self.assertNotIn("Checking the selected PostgreSQL", result.stdout)

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

    def test_default_container_matches_existing_v3_store(self):
        for target in ("web", "desktop"):
            with self.subTest(target=target):
                result, commands = self.run_launcher("disk", use_defaults=True, target=target)
                self.assertIn(["start", "helix-quarry-v3-dev"], commands)
                self.assertFalse(any(c[0] == "create" for c in commands))
                self.assertIn("could not start Helix", result.stderr)

    def test_config_with_comments_and_whitespace(self):
        config = (ROOT / "backend/helix.toml").read_text()
        config = config.replace("[local.dev]", "  [local.dev] # local runtime")
        config = config.replace('tag = "latest"', 'tag = "latest" # descriptive tag')
        result, commands = self.run_launcher("disk", config=config)
        self.assertIn(["start", CONTAINER], commands)
        self.assertIn("could not start Helix", result.stderr)

    def test_missing_digest_fails_before_container_changes(self):
        config = '[local.dev]\nimage = "example/helix"\ntag = "latest"\n'
        result, commands = self.run_launcher("disk", config=config)
        self.assertIn("could not read the local Helix image", result.stderr)
        self.assertEqual(commands, [["info"]])

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
