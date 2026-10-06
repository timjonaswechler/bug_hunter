"""Shared process-independent acceptance helpers; never launches on import."""
from contextlib import nullcontext
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
CLI = ROOT / "target/debug/woodpecker"


def until(callback, timeout=90, description="condition did not become true"):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = callback()
        if result:
            return result
        time.sleep(0.025)
    raise AssertionError(description)


def wait_until(callback, description, timeout=15):
    return until(callback, timeout=timeout, description=description)


def spawn_server(directory, log, shutdown_seconds=10):
    """Start only the server; the caller owns readiness, shutdown and cleanup."""
    return subprocess.Popen(
        [str(CLI), "--address", "127.0.0.1:0", "server", "start",
         "--artifact-dir", str(directory / "artifacts"),
         "--shutdown-seconds", str(shutdown_seconds)],
        cwd=ROOT, stdout=subprocess.PIPE, stderr=log, text=True,
    )


def cli_call(address, *args, success=True, timeout=40, journal=None,
             journal_keys=("arguments", "exit"), journal_lock=None, allow_empty=False):
    """One CLI call; no retry, tick, lifecycle change or hidden result polling."""
    result = subprocess.run(
        [str(CLI), "--address", address, *args], cwd=ROOT,
        capture_output=True, text=True, timeout=timeout,
    )
    if journal is not None:
        argument_key, status_key = journal_keys
        with journal_lock if journal_lock is not None else nullcontext():
            journal.write(json.dumps({argument_key: args, status_key: result.returncode,
                                      "stdout": result.stdout, "stderr": result.stderr}) + "\n")
            journal.flush()
    if success:
        assert result.returncode == 0, (args, result.stdout, result.stderr)
    else:
        assert result.returncode != 0, (args, result.stdout, result.stderr)
    if allow_empty and not result.stdout:
        return None
    return json.loads(result.stdout)
