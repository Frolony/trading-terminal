// ci-issue-report.cjs — alert-only issue filing for the N6.2 E2E CI workflow (§8.6).
//
// Modes:
//   node ci-issue-report.cjs                 — failure mode: parse the Playwright JSON results
//                                            (RESULTS_JSON env) and create-or-comment on the
//                                            `e2e-regression` issue for UNDER_TEST_SHA.
//   node ci-issue-report.cjs --close-on-green — recovery mode: close any open
//                                            `e2e-regression` issue for UNDER_TEST_SHA.
//
// Failure detection is keyed on the REAL Playwright JSON reporter shape (1.64.1,
// ground truth from the 2026-10-23 ~19:00 CST distinguishing experiment):
//   - spec.ok (boolean) === false marks a failed-after-retries spec;
//   - spec.tests[].status === 'unexpected' is the per-test signal;
//   - result-level statuses are 'passed'|'failed'|'timedOut'|'interrupted'|'skipped';
//   - top-level json.errors carries runner-level errors (setup/config/hook failures).
// A legacy hand-authored seed may carry spec.outcome ('unexpected') instead;
// that shape is still honored. The old code keyed ONLY on spec.outcome, which
// real reporter output does not contain — the reporter was blind to every real
// failure and runs #9/#10 failed without filing anything. Never key on
// spec.outcome alone again.
//
// Env: GH_TOKEN (via the workflow), UNDER_TEST_SHA, RUN_URL, BASE_URL, DEPLOY_URL, RESULTS_JSON.
// The workflow only invokes failure mode when the e2e job failed, so failure mode
// ALWAYS files an alert — spec failures, a runner-level failure with zero parsed
// spec failures (the old silent-drop), or a missing/unusable results file. An
// alert that cannot be filed exits 2 with the full report on stdout so the loss
// is visible in the workflow log (the material report is never silently dropped).

'use strict';

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');

const LABEL = 'e2e-regression';

function shortSha(sha) {
  return String(sha || '').slice(0, 8);
}

function truncate(s, n) {
  return String(s).split('\n').slice(0, 25).join('\n').slice(0, n);
}

// A spec failed iff (real JSON) ok === false, else (legacy seed) outcome ===
// 'unexpected', else any test with status 'unexpected'. Flaky and skipped are
// never failures.
function specFailed(spec) {
  if (typeof spec.ok === 'boolean') return spec.ok === false;
  if (typeof spec.outcome === 'string') return spec.outcome === 'unexpected';
  return (spec.tests || []).some((t) => t && t.status === 'unexpected');
}

function specErrorMessage(spec) {
  const attempts = ((spec.tests || [])[0] || {}).results || [];
  const last = attempts[attempts.length - 1] || {};
  const fromError = last.error && last.error.message;
  const fromErrors = Array.isArray(last.errors) && last.errors.length
    ? last.errors.map((e) => (e && e.message) || String(e)).join('\n---\n')
    : null;
  return truncate(fromError || fromErrors || 'no error message captured', 2000);
}

// Walk the Playwright JSON reporter tree and collect failed specs.
// Returns [{file, title, error}].
function collectFailures(playwrightJson) {
  const failures = [];
  function visitSuite(suite) {
    for (const spec of suite.specs || []) {
      if (specFailed(spec)) {
        failures.push({
          file: spec.file || suite.file || '(unknown file)',
          title: spec.title || '(untitled)',
          error: specErrorMessage(spec),
        });
      }
    }
    for (const child of suite.suites || []) visitSuite(child);
  }
  for (const suite of playwrightJson.suites || []) visitSuite(suite);
  return failures;
}

// Top-level runner errors (Playwright json.errors): setup/config/hook failures
// that never attach to a spec. Returns [message, ...], truncated.
function collectTopLevelErrors(playwrightJson) {
  const errors = playwrightJson.errors || [];
  return errors
    .map((e) => truncate((e && e.message) || String(e), 1500))
    .filter((m) => m.length > 0);
}

// json.stats when present: {expected, unexpected, flaky, skipped}. Returns null
// when the results JSON carries no stats block.
function collectStats(playwrightJson) {
  const s = playwrightJson.stats;
  if (!s || typeof s !== 'object') return null;
  const n = (k) => (typeof s[k] === 'number' ? s[k] : 0);
  return { expected: n('expected'), unexpected: n('unexpected'), flaky: n('flaky'), skipped: n('skipped') };
}

function statsLine(stats) {
  if (!stats) return null;
  const total = stats.expected + stats.unexpected + stats.flaky + stats.skipped;
  return `- Suite stats: ${total} total — ${stats.expected} expected, ${stats.unexpected} unexpected, ${stats.flaky} flaky, ${stats.skipped} skipped`;
}

function preamble({ sha, deployUrl, baseUrl, runUrl }) {
  return [
    `## E2E regression failure — \`${shortSha(sha)}\``,
    '',
    `**Alert-only (§8.6):** this does not block pushes, deployments, or production promotion.`,
    `Fix the failures; the next green run closes this issue automatically.`,
    '',
    `- Commit under test: \`${sha}\``,
    `- Suite ran against (BASE_URL): ${baseUrl}`,
    deployUrl ? `- Deployment URL (pinned; SSO-gated — not directly exercised): ${deployUrl}` : null,
    `- Workflow run: ${runUrl}`,
  ].filter((l) => l !== null);
}

function footer() {
  return 'Artifacts (Playwright traces + video, retain-on-failure) are attached to the workflow run.';
}

function topErrorsSection(topErrors) {
  if (!topErrors || topErrors.length === 0) return [];
  return [
    '',
    '### Runner-level errors (top-level `errors` in the results JSON)',
    '',
    ...topErrors.map((e, i) => `${i + 1}. \`\`\`\n${e}\n\`\`\``),
  ];
}

function buildFailureBody({ failures, sha, deployUrl, baseUrl, runUrl, topErrors, stats }) {
  const total = stats
    ? stats.expected + stats.unexpected + stats.flaky + stats.skipped
    : 19;
  const lines = failures.map(
    (f, i) => `${i + 1}. **${f.title}** (\`${f.file}\`)\n\`\`\`\n${f.error}\n\`\`\``
  );
  return [
    ...preamble({ sha, deployUrl, baseUrl, runUrl }),
    `- Failed tests: ${failures.length} of ${total} (T1–T6, Chromium)`,
    statsLine(stats),
    '',
    '### Failing tests',
    '',
    ...lines,
    ...topErrorsSection(topErrors),
    '',
    footer(),
  ]
    .filter((l) => l !== null)
    .join('\n');
}

// The e2e job failed but the results JSON has no failed specs: the failure is
// at the runner level (install/config/timeout before specs ran, or a hook that
// aborted the run). This used to exit 0 without filing — the silent drop that
// hid runs #9/#10. It now files, honestly labelled.
function buildRunnerFailureBody({ sha, deployUrl, baseUrl, runUrl, topErrors, stats }) {
  return [
    ...preamble({ sha, deployUrl, baseUrl, runUrl }),
    `- Failed tests: 0 parsed of ${stats ? stats.expected + stats.unexpected + stats.flaky + stats.skipped : 'unknown'} (the job itself failed — see below)`,
    statsLine(stats),
    '',
    '### Runner-level failure — no failed specs parsed',
    '',
    'The E2E job failed but the results JSON contains no failed specs, so there',
    'is nothing spec-level to list. The failure is at the runner level (dependency',
    'install, Chromium install, config/setup error, or a timeout that killed the',
    'run before specs completed). Start from the workflow log and the run artifacts.',
    ...topErrorsSection(topErrors),
    ...(topErrors && topErrors.length === 0
      ? ['', 'No top-level `errors` were captured in the results JSON — the workflow log is the source of truth.']
      : []),
    '',
    footer(),
  ]
    .filter((l) => l !== null)
    .join('\n');
}

// RESULTS_JSON missing or unparseable: the job failed before producing usable
// results (artifact upload/download failure, runner killed mid-write). File
// anyway — an alert with no test list beats a silent non-file.
function buildMissingResultsBody({ loadError, sha, deployUrl, baseUrl, runUrl }) {
  return [
    ...preamble({ sha, deployUrl, baseUrl, runUrl }),
    '',
    '### Results unavailable',
    '',
    `The E2E job failed before usable results could be parsed: ${loadError}.`,
    'Start from the workflow log; the results artifact may be absent or truncated.',
    '',
    footer(),
  ].join('\n');
}

function gh(args, input) {
  return execFileSync('gh', args, {
    input,
    encoding: 'utf8',
    stdio: ['pipe', 'pipe', 'pipe'],
  }).trim();
}

function findOpenIssue(sha) {
  const short = shortSha(sha);
  let out;
  try {
    out = gh(['issue', 'list', '--label', LABEL, '--state', 'open', '--json', 'number,title']);
  } catch (e) {
    throw new Error(`gh issue list failed: ${e.message}`);
  }
  const issues = JSON.parse(out || '[]');
  return issues.find((i) => String(i.title).includes(short)) || null;
}

function fileOrComment(body, sha, detail) {
  try {
    const existing = findOpenIssue(sha);
    if (existing) {
      gh(['issue', 'comment', String(existing.number), '--body-file', '-'], body);
      console.log(`Commented on existing issue #${existing.number} (${detail}).`);
    } else {
      const created = gh(
        ['issue', 'create', '--label', LABEL, '--title', `[e2e] E2E regression failure — ${shortSha(sha)}`, '--body-file', '-'],
        body
      );
      console.log(`Created issue: ${created} (${detail}).`);
    }
  } catch (e) {
    console.error(`ISSUE FILING FAILED: ${e.message}`);
    console.error('--- material report (preserved in this log) ---');
    console.error(body);
    process.exit(2);
  }
}

function failureMode() {
  const ctx = {
    sha: process.env.UNDER_TEST_SHA || 'unknown',
    deployUrl: process.env.DEPLOY_URL || '',
    baseUrl: process.env.BASE_URL || 'https://gecko-solana-terminal.vercel.app',
    runUrl: process.env.RUN_URL || '(no run URL)',
  };
  const resultsPath = process.env.RESULTS_JSON;
  let data = null;
  let loadError = null;
  if (!resultsPath) {
    loadError = 'RESULTS_JSON env var not set';
  } else if (!fs.existsSync(resultsPath)) {
    loadError = `results file not found: ${resultsPath}`;
  } else {
    try {
      data = JSON.parse(fs.readFileSync(resultsPath, 'utf8'));
    } catch (e) {
      loadError = `results file is not valid JSON: ${e.message}`;
    }
  }

  let body;
  let detail;
  if (loadError) {
    body = buildMissingResultsBody({ loadError, ...ctx });
    detail = 'results unavailable';
  } else {
    const failures = collectFailures(data);
    const topErrors = collectTopLevelErrors(data);
    const stats = collectStats(data);
    if (failures.length > 0) {
      body = buildFailureBody({ failures, topErrors, stats, ...ctx });
      detail = `${failures.length} failing tests`;
    } else {
      body = buildRunnerFailureBody({ topErrors, stats, ...ctx });
      detail = 'runner-level failure, zero parsed spec failures';
    }
  }
  fileOrComment(body, ctx.sha, detail);
}

function closeOnGreen() {
  const sha = process.env.UNDER_TEST_SHA || 'unknown';
  const runUrl = process.env.RUN_URL || '(no run URL)';
  try {
    const existing = findOpenIssue(sha);
    if (!existing) {
      console.log(`No open ${LABEL} issue for ${shortSha(sha)} — nothing to close.`);
      return;
    }
    gh(['issue', 'close', String(existing.number), '--comment', `Green E2E run for \`${shortSha(sha)}\`: ${runUrl} — closing the alert.`]);
    console.log(`Closed issue #${existing.number} (green run).`);
  } catch (e) {
    console.error(`ISSUE CLOSE FAILED: ${e.message}`);
    process.exit(2);
  }
}

if (require.main === module) {
  if (process.argv.includes('--close-on-green')) closeOnGreen();
  else failureMode();
}

module.exports = {
  collectFailures,
  collectTopLevelErrors,
  collectStats,
  specFailed,
  buildFailureBody,
  buildRunnerFailureBody,
  buildMissingResultsBody,
  shortSha,
  LABEL,
};
