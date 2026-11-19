"""Exercise PostgreSQL startup and API supervision using isolated command stubs."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]

# Any Docker call is recorded so tests can prove the launcher never uses it.
DOCKER_STUB = r'''#!/usr/bin/env python3
import json, os, pathlib, sys
folder = pathlib.Path(os.environ["QUARRY_TEST_FOLDER"])
with (folder / "docker.jsonl").open("a") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
sys.exit(0)
'''


class LauncherTests(unittest.TestCase):
    def run_launcher(
        self, postgres_status=0, start_status=0, target="web",
        backend_status=17, with_docker=True,
    ):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            if with_docker:
                docker = folder / "docker"
                docker.write_text(DOCKER_STUB)
                docker.chmod(0o755)
            for name in ("node",):
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
            # Record the backend command and its environment, then fail startup.
            (folder / "cargo").write_text(
                '#!/bin/sh\nprintf "%s\\n" "$*" >> "$QUARRY_TEST_FOLDER/cargo-commands"\n'
                'env > "$QUARRY_TEST_FOLDER/cargo-env"\n'
                'exit ' + str(backend_status) + "\n")
            (folder / "cargo").chmod(0o755)
            (folder / "npm").write_text(
                '#!/bin/sh\ntouch "$QUARRY_TEST_FOLDER/ui-started"\nexit 0\n')
            (folder / "npm").chmod(0o755)
            (folder / "curl").write_text("#!/bin/sh\nexit 1\n")
            (folder / "curl").chmod(0o755)
            launcher_root = folder / "repo"
            (launcher_root / "backend").mkdir(parents=True)
            (launcher_root / "frontend").mkdir()
            (launcher_root / "quarry").write_text((ROOT / "quarry").read_text())
            # Use only the stub folder plus system tools, so a host Docker is never found.
            path = f"{folder}:/usr/bin:/bin"
            result = subprocess.run(
                ["bash", str(launcher_root / "quarry"), target],
                cwd=launcher_root,
                env={
                    **os.environ,
                    "PATH": path,
                    "QUARRY_POSTGRES_BIN": str(folder),
                    "QUARRY_POSTGRES_DATA_DIR": str(folder),
                    "QUARRY_TEST_POSTGRES_STATUS": str(postgres_status),
                    "QUARRY_TEST_START_STATUS": str(start_status),
                    "QUARRY_TEST_FOLDER": str(folder),
                },
                capture_output=True,
                text=True,
                timeout=30,
            )
            self.assertFalse((folder / "docker.jsonl").exists(), "launcher invoked Docker")
            postgres_commands = [json.loads(line) for line in
                                 (folder / "postgres.jsonl").read_text().splitlines()]
            self.assertEqual(any("start" in c for c in postgres_commands), postgres_status == 3)
            self.assertFalse(any("initdb" in c or "stop" in c for c in postgres_commands))
            cargo_log = folder / "cargo-commands"
            cargo_commands = cargo_log.read_text().splitlines() if cargo_log.exists() else []
            cargo_env_file = folder / "cargo-env"
            cargo_env = cargo_env_file.read_text() if cargo_env_file.exists() else ""
            self.assertFalse((folder / "ui-started").exists())
        return result, cargo_commands, cargo_env

    def test_stopped_postgres_starts_for_both_targets(self):
        for target in ("web", "desktop"):
            with self.subTest(target=target):
                result, cargo, _ = self.run_launcher(postgres_status=3, target=target)
                self.assertIn("Starting local PostgreSQL", result.stdout)
                self.assertEqual(cargo, ["run --locked"])

    def test_postgres_start_failure_stops_before_the_api(self):
        result, cargo, _ = self.run_launcher(postgres_status=3, start_status=1)
        self.assertEqual(result.returncode, 1)
        self.assertIn("could not start the existing PostgreSQL", result.stderr)
        self.assertEqual(cargo, [])

    def test_uninspectable_postgres_is_not_started(self):
        result, cargo, _ = self.run_launcher(postgres_status=4)
        self.assertEqual(result.returncode, 1)
        self.assertIn("could not inspect the existing PostgreSQL", result.stderr)
        self.assertEqual(cargo, [])

    def test_backend_failure_exits_without_starting_the_ui(self):
        for target in ("web", "desktop"):
            for status in (0, 17):
                with self.subTest(target=target, status=status):
                    result, cargo, _ = self.run_launcher(backend_status=status, target=target)
                    self.assertEqual(result.returncode, status or 1)
                    self.assertEqual(cargo, ["run --locked"])
                    self.assertIn("Axum API exited before becoming ready", result.stderr)

    def test_startup_requires_no_docker_or_graph_store(self):
        for with_docker in (False, True):
            with self.subTest(with_docker=with_docker):
                result, cargo, cargo_env = self.run_launcher(with_docker=with_docker)
                self.assertNotIn("required command not found", result.stderr)
                self.assertEqual(cargo, ["run --locked"])
                self.assertIn("QUARRY_TEMPLATE_PREVIEW_RENDER_URL=", cargo_env)
                self.assertNotIn("Docker", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
