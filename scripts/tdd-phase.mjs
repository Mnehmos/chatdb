import { spawnSync } from 'node:child_process';
import process from 'node:process';

const phases = new Set(['red', 'green']);
const stacks = new Set(['frontend', 'rust', 'python']);

const usage = `ChatDB TDD phase runner

Usage:
  node scripts/tdd-phase.mjs <phase> <stack> [target]

Phases:
  red     Run the targeted test command and expect it to fail.
  green   Run the targeted test command and expect it to pass.

Stacks:
  frontend  Runs TypeScript preflight plus Vitest target.
  rust      Runs cargo test for the target.
  python    Runs pytest for the target.

Examples:
  node scripts/tdd-phase.mjs red frontend src/components/loop/LoopControls.test.tsx
  node scripts/tdd-phase.mjs green frontend src/stores/problemStore.test.ts
  node scripts/tdd-phase.mjs red rust verification::tests
  node scripts/tdd-phase.mjs green python sidecar/tests/test_main_app.py
`;

function resolveExecutable(command) {
  if (process.platform === 'win32' && (command === 'npm' || command === 'npx')) {
    return `${command}.cmd`;
  }

  return command;
}

function shouldUseShell(command) {
  return process.platform === 'win32' && (command === 'npm' || command === 'npx');
}

function formatCommand(command, args) {
  return [command, ...args].join(' ');
}

function runCommand(step, expectSuccess) {
  console.log(`\n[${step.label}] ${formatCommand(step.command, step.args)}`);

  const useShell = shouldUseShell(step.command);

  const result = spawnSync(useShell ? step.command : resolveExecutable(step.command), step.args, {
    cwd: step.cwd ?? process.cwd(),
    stdio: 'inherit',
    shell: useShell,
  });

  if (result.error) {
    console.error(`Failed to launch ${step.label}: ${result.error.message}`);
    process.exit(1);
  }

  const exitCode = result.status ?? 1;
  const succeeded = exitCode === 0;

  if (succeeded !== expectSuccess) {
    if (expectSuccess) {
      console.error(`${step.label} failed with exit code ${exitCode}.`);
    } else {
      console.error(`${step.label} passed unexpectedly. Red evidence was not established.`);
    }

    process.exit(expectSuccess ? exitCode : 1);
  }
}

function buildPlan(stack, target) {
  switch (stack) {
    case 'frontend': {
      const testArgs = ['run', 'test:run'];
      if (target) {
        testArgs.push('--', target);
      }

      return {
        preflight: [
          {
            label: 'Frontend preflight',
            command: 'npx',
            args: ['tsc', '--noEmit'],
          },
        ],
        target: {
          label: 'Frontend target',
          command: 'npm',
          args: testArgs,
        },
      };
    }

    case 'rust':
      return {
        preflight: [],
        target: {
          label: 'Rust target',
          command: 'cargo',
          args: target ? ['test', target] : ['test'],
        },
      };

    case 'python':
      return {
        preflight: [],
        target: {
          label: 'Python target',
          command: 'python',
          args: target ? ['-m', 'pytest', target] : ['-m', 'pytest'],
        },
      };

    default:
      console.error(`Unsupported stack: ${stack}`);
      process.exit(1);
  }
}

const args = process.argv.slice(2);

if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log(usage);
  process.exit(0);
}

const [phase, stack, target] = args;

if (!phases.has(phase)) {
  console.error(`Invalid phase: ${phase}\n`);
  console.log(usage);
  process.exit(1);
}

if (!stacks.has(stack)) {
  console.error(`Invalid stack: ${stack}\n`);
  console.log(usage);
  process.exit(1);
}

const plan = buildPlan(stack, target);

for (const step of plan.preflight) {
  runCommand(step, true);
}

runCommand(plan.target, phase === 'green');

if (phase === 'red') {
  console.log('\nRed confirmed: the targeted test command failed as expected.');
} else {
  console.log('\nGreen confirmed: the targeted test command passed.');
}
