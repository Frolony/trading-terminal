// ci-issue-report.cjs — alert-only issue filing for the N6.2 E2E CI workflow (§8.6).
//
// Modes:
//   node ci-issue-report.cjs                 — failure mode: parse the Playwright JSON results
//                                            (RESULTS_JSON env) and create-or-comment on the
//                                            `e2e-regression` issue for UNDER_TEST_SHA.
//   node ci-issue-report.cjs --close-on-green — recovery mode: close any open
//                                            `e2e-regression` issue for UNDER_TEST_SHA.
//
// Env: GH_TOKEN (via the workflow), UNDER_TEST_SHA, RUN_URL, BASE_URL, DEPLOY_URL, RESULTS_JSON.
// The gh CLI is used for all GitHub writes. gh failures print the full report to stdout and
// exit 2 so the loss of the alert is visible in the workflow log (the material report is
// never silently dropped).

'use strict';

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');

const LABEL = 'e2e-regression';

function shortSha(sha) {
  return String(sha || '').slice(0, 8);
}

// Walk the Playwright JSON reporter tree and collect specs whose final outcome is
// 'unexpected' (failed, after the pack's one retry). Returns [{file, title, error}].
function collectFailures(playwrightJson) {
  const failures = [];
  function visitSuite(suite) {
    for (const spec of suite.specs || []) {
      if (spec.outcome === 'unexpected') {
        const attempts = ((spec.tests || [])[0] || {}).results || [];
        const last = attempts[attempts.length - 1] || {};
        const raw = (last.error && last.error.message) || 'no error message captured';
        failures.push({
          file: spec.file || suite.file || '(unknown file)',
          title: spec.title || '(untitled)',
          error: String(raw).split('\n').slice(0, 25).join('\n').slice(0, 2000),
        });
      }
    }
    for (const child of suite.suites || []) visitSuite(child);
  }
  for (const suite of playwrightJson.suites || []) visitSuite(suite);
  return failures;
}

function buildFailureBody({ failures, sha, deployUrl, baseUrl, runUrl }) {
  const lines = failures.map(
    (f, i) => `${i + 1}. **${f.title}** (\`${f.file}\`)\n\`\`\`\n${f.error}\n\`\`\``
  );
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
    `- Failed tests: ${failures.length} of 19 (T1–T6, Chromium)`,
    '',
    '### Failing tests',
    '',
    ...lines,
    '',
    'Artifacts (Playwright traces + video, retain-on-failure) are attached to the workflow run.',
  ]
    .filter((l) => l !== null)
    .join('\n');
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

function failureMode() {
  const resultsPath = process.env.RESULTS_JSON;
  if (!resultsPath || !fs.existsSync(resultsPath)) {
    console.error(`RESULTS_JSON missing or unreadable: ${resultsPath}`);
    process.exit(1);
  }
  const data = JSON.parse(fs.readFileSync(resultsPath, 'utf8'));
  const failures = collectFailures(data);
  if (failures.length === 0) {
    console.log('No unexpected spec outcomes in the results JSON — nothing to file.');
    return;
  }
  const env = {
    failures,
    sha: process.env.UNDER_TEST_SHA || 'unknown',
    deployUrl: process.env.DEPLOY_URL || '',
    baseUrl: process.env.BASE_URL || 'https://gecko-solana-terminal.vercel.app',
    runUrl: process.env.RUN_URL || '(no run URL)',
  };
  const body = buildFailureBody(env);
  try {
    const existing = findOpenIssue(env.sha);
    if (existing) {
      gh(['issue', 'comment', String(existing.number), '--body-file', '-'], body);
      console.log(`Commented on existing issue #${existing.number} (${failures.length} failing tests).`);
    } else {
      const created = gh(
        ['issue', 'create', '--label', LABEL, '--title', `[e2e] E2E regression failure — ${shortSha(env.sha)}`, '--body-file', '-'],
        body
      );
      console.log(`Created issue: ${created} (${failures.length} failing tests).`);
    }
  } catch (e) {
    console.error(`ISSUE FILING FAILED: ${e.message}`);
    console.error('--- material report (preserved in this log) ---');
    console.error(body);
    process.exit(2);
  }
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

module.exports = { collectFailures, buildFailureBody, shortSha, LABEL };
