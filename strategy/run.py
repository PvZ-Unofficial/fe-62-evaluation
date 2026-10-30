"""Batch PE evaluations with an exclusive machine-wide budget and isolated outputs."""
import argparse
import contextlib
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import uuid


@contextlib.contextmanager
def cpu_budget():
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.CreateMutexW.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_wchar_p]
    k.CreateMutexW.restype = ctypes.c_void_p
    k.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    k.ReleaseMutex.argtypes = k.CloseHandle.argtypes = [ctypes.c_void_p]
    handle = k.CreateMutexW(None, False, 'Local\\PvZ_MLP_CPU_BUDGET')
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    acquired = False
    try:
        print('Waiting for shared simulation budget', flush=True)
        result = k.WaitForSingleObject(handle, 0xFFFFFFFF)
        if result not in (0, 0x80):
            raise RuntimeError(f'mutex wait failed: {result}')
        acquired = True
        yield
    finally:
        if acquired:
            k.ReleaseMutex(handle)
        k.CloseHandle(handle)


def run(args):
    expected_seeds = list(range(args.seeds, args.seeds + args.trials))
    if args.seed_file:
        expected_seeds = [int(s) for s in Path(args.seed_file).read_text(encoding='utf-8').split()]
        if len(expected_seeds) != args.trials or len(set(expected_seeds)) != args.trials or any(not 0 <= s < 2**32 for s in expected_seeds):
            raise ValueError('seed file must contain exactly trials unique uint32 seeds')
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    script = Path(args.script).resolve()
    workspace = Path(__file__).resolve().parent.parent / 'RustVsZombies'
    env = os.environ.copy()
    env.update(SIX_TRIALS=str(args.trials), SIX_SUN='8000', SIX_CHAIN_ROUNDS=str(args.rounds),
               SIX_TRACE_SAMPLES=str(args.trace), SIX_DETAIL=str(args.detail),
               SIX_DIAGNOSTICS='0', SIX_DYNAMIC_ROOTS='1',
               SIX_ROUND_TIMEOUT_FRAMES=str(args.round_timeout_frames))
    if args.policy is not None:
        env['SIX_POLICY'] = args.policy
    if args.model is not None:
        env['SIX_POLICY_MODEL'] = str(Path(args.model).resolve())
    for item in args.env:
        key, value = item.split('=', 1)
        env[key] = value
    if args.seed_file:
        env['SIX_ROOT_SEEDS_FILE'] = str(Path(args.seed_file).resolve())
    else:
        env.pop('SIX_ROOT_SEEDS_FILE', None)
    (output / 'seeds.json').write_text(json.dumps(expected_seeds), encoding='utf-8')
    if args.budget_parent is not None and args.budget_parent != os.getppid():
        raise ValueError('Inherited CPU budget must belong to the direct batch parent')
    with (contextlib.nullcontext() if args.budget_parent is not None else cpu_budget()):
        if args.runner:
            runner = Path(args.runner).resolve()
        else:
            command = [str(workspace / 'target/debug/rsvz.exe'), 'run-pe', '--script-crate', str(script),
                       '--release', '--pe-toolchain', args.pe_toolchain, '--build-only']
            if args.pgo_use:
                command += ['--pgo-use', str(Path(args.pgo_use).resolve())]
            built = subprocess.run(command, cwd=workspace, env=env, text=True, capture_output=True)
            (output / 'build.log').write_text(built.stdout + built.stderr, encoding='utf-8')
            built.check_returncode()
            match = re.search(r'PE runner EXE: (.+)', built.stdout + built.stderr)
            if not match:
                raise RuntimeError('Build did not report runner path')
            runner = Path(match[1].strip())
        run_id = uuid.uuid4().hex
        config = dict(schema_version=4, run_id=run_id, threads=min(args.workers,args.trials),
                      seed_mode=dict(kind='base', seed=args.seeds),
                      limits=dict(max_wall_ms=int(args.seconds * 1000) if args.seconds else None,
                                  max_sim_frames=None, max_levels=None), profile=args.profile,
                      output_path=str(output / 'results.json'), raw_result_path=str(output / 'runtime.json'),
                      diagnostics=dict(options=dict(mode='file', log_file=None, level=args.log_level, progress_interval_ms=None),
                                       run_id=run_id, following=False, worker=0, source='script',
                                       default_file=str(output / 'trace')))
        config_path = output / 'config.json'
        config_path.write_text(json.dumps(config), encoding='utf-8')
        (output / 'config.saved.json').write_text(json.dumps(config, indent=2), encoding='utf-8')
        (output / 'runner.json').write_text(json.dumps(dict(path=str(runner), sha256=hashlib.sha256(runner.read_bytes()).hexdigest())), encoding='utf-8')
        env['RSVZ_PE_RUN_CONFIG_PATH'] = str(config_path)
        (output / 'invocation.json').write_text(json.dumps(vars(args), indent=2), encoding='utf-8')
        start = time.monotonic()
        with (output / 'console.log').open('w', encoding='utf-8') as log:
            result = subprocess.run([str(runner)], cwd=workspace, env=env, stdout=log, stderr=log)
        elapsed = time.monotonic() - start
        (output / 'timing.json').write_text(json.dumps(dict(seconds=elapsed, exit_code=result.returncode)), encoding='utf-8')
        artifact = output / 'results.json'
        report = json.loads(artifact.read_text(encoding='utf-8')) if artifact.exists() else {'roots': []}
        # The host deliberately withholds merged artifacts on limit exits, but the
        # before-exit hooks still log complete per-root records. Recover those first.
        recorded = {root['seed']: root for root in report['roots']}
        for trace in output.glob('trace.*.jsonl'):
            for line in trace.read_text(encoding='utf-8').splitlines():
                try:
                    message = json.loads(line).get('payload', {}).get('message', '')
                    if message.startswith('six_two_root '):
                        root = json.loads(message[len('six_two_root '):])
                        recorded[root['seed']] = root
                except json.JSONDecodeError:
                    pass  # A process crash can leave a final partial log line.
        roots = report['roots'] = list(recorded.values())
        seen = {root['seed'] for root in roots}
        # A budget can expire before hooks install, or before the next assigned root starts.
        # Preserve the preselected cohort rather than silently dropping slow/missing roots.
        if seen - set(expected_seeds):
            raise ValueError('runner reported unexpected seeds')
        for seed in expected_seeds:
            if seed not in seen:
                roots.append(dict(seed=seed, status='censored', n=0, round=1, cause='unreported_or_not_started',
                                  wave=0, time=0, row=0, col=0, kind=-1, sun=None, frames=0, clock=None))
        roots.sort(key=lambda root: root['seed'])
        artifact.write_text(json.dumps(report), encoding='utf-8')
        (output / 'roots.jsonl').write_text(''.join(json.dumps(root) + '\n' for root in roots), encoding='utf-8')
        print(json.dumps(dict(output=str(output), seconds=elapsed, exit_code=result.returncode)), flush=True)
        result.check_returncode()


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--script', default=str(Path(__file__).resolve().parent))
    p.add_argument('--runner', help='Already built runner; skip build for repeated model batches')
    p.add_argument('--pe-toolchain', choices=('clang', 'clang-thin-lto', 'clang-full-lto', 'msvc'), default='clang-thin-lto')
    p.add_argument('--pgo-use', help='Optional merged PGO .profdata file for building')
    p.add_argument('--output', required=True)
    p.add_argument('--seeds', type=int, default=80000)
    p.add_argument('--seed-file', help='Explicit unique uint32 root seeds, one per line')
    p.add_argument('--trials', type=int, default=200)
    p.add_argument('--workers', type=int, choices=range(1, (os.cpu_count() or 12) + 1), default=12)
    p.add_argument('--rounds', type=int, default=0, help='0: until real failure; positive: regression cap')
    p.add_argument('--round-timeout-frames', type=int, default=0, help='0: no per-round timeout; positive: censored regression limit')
    p.add_argument('--seconds', type=float, default=0, help='Optional safety budget; incomplete roots are censored')
    p.add_argument('--trace', type=int, default=0)
    p.add_argument('--detail', type=int, choices=(0, 1), default=0)
    p.add_argument('--log-level', choices=('info', 'error'), default='info')
    p.add_argument('--policy')
    p.add_argument('--profile', action='store_true', help='Measure script versus native frame time')
    p.add_argument('--model')
    p.add_argument('--env', action='append', default=[])
    p.add_argument('--budget-parent', type=int, help=argparse.SUPPRESS)
    run(p.parse_args())
