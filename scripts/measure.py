#!/usr/bin/env python3
"""Sample the entire launched process tree. No third-party Python packages."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time


def processes():
    result = {}
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / 'stat').read_text().rsplit(')', 1)[1].split()
            result[int(entry.name)] = (int(stat[1]), int(stat[11]) + int(stat[12]))
        except (OSError, ValueError, IndexError):
            continue
    return result


def sample(root):
    table = processes()
    members = {root}
    while True:
        expanded = members | {pid for pid, (parent, _) in table.items() if parent in members}
        if expanded == members:
            break
        members = expanded
    rss = pss = ticks = count = 0
    pss_readable = True
    for pid in members:
        if pid not in table:
            continue
        ticks += table[pid][1]
        count += 1
        try:
            status = Path(f'/proc/{pid}/status').read_text()
            rss += next((int(line.split()[1]) for line in status.splitlines() if line.startswith('VmRSS:')), 0)
            rollup = Path(f'/proc/{pid}/smaps_rollup').read_text()
            pss += next((int(line.split()[1]) for line in rollup.splitlines() if line.startswith('Pss:')), 0)
        except (OSError, ValueError):
            pss_readable = False
    return {'rss_mib': round(rss / 1024, 1), 'pss_mib': round(pss / 1024, 1) if pss_readable else None,
            'processes': count, 'ticks': ticks}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', type=int, default=20)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command:
        parser.error('provide a command after --')
    start = time.monotonic()
    process = subprocess.Popen(command, start_new_session=True)
    readings = []
    first_window = None
    try:
        while time.monotonic() - start < args.seconds:
            if process.poll() is not None:
                raise SystemExit(f'App exited ({process.returncode}). Close any existing instance before measuring.')
            reading = sample(process.pid)
            reading['seconds'] = round(time.monotonic() - start, 2)
            readings.append(reading)
            # X11/XWayland only. A pure Wayland window is deliberately not estimated.
            if first_window is None and shutil.which('xdotool') and os.environ.get('DISPLAY'):
                windows = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(process.pid)],
                                         capture_output=True, check=False)
                if windows.returncode == 0:
                    first_window = round(time.monotonic() - start, 3)
            time.sleep(0.25)
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
    elapsed = readings[-1]['seconds'] if readings else 0
    cpu = readings[-1]['ticks'] / os.sysconf('SC_CLK_TCK') / elapsed * 100 if elapsed else 0
    report = {'first_visible_window_seconds_x11': first_window, 'duration_seconds': elapsed,
              'tree_cpu_percent_one_core': round(cpu, 2), 'final': readings[-1] if readings else None,
              'peak_tree_rss_mib': max((r['rss_mib'] for r in readings), default=0), 'samples': readings}
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
