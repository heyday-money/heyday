const assert = require('node:assert/strict');
const ensureRelease = require('./github-release.cjs');

async function main() {
  const context = { repo: { owner: 'example', repo: 'app' } };
  const existing = { id: 7, tag_name: 'v1.0.0-beta.2', body: 'Keep my notes', draft: true };
  let releases = [existing], creates = 0, creation;
  const github = {
    paginate: { async *iterator() { yield { data: releases }; } },
    rest: { repos: {
      listReleases() {},
      async createRelease(input) { creates++; creation = input; return { data: { id: 8, ...input } }; },
    } },
  };
  const args = { github, context, tag: existing.tag_name, prerelease: true };
  assert.equal(await ensureRelease(args), existing);
  assert.equal(creates, 0);
  releases = [];
  assert.equal((await ensureRelease(args)).id, 8);
  assert.equal(creation.prerelease, true);
  assert.equal(creation.draft, false);
  assert.equal(creation.tag_name, existing.tag_name);
  github.rest.repos.createRelease = async () => {
    releases = [existing];
    throw Object.assign(new Error('Concurrent creation'), { status: 422 });
  };
  assert.equal(await ensureRelease(args), existing);
  releases = [];
  github.rest.repos.createRelease = async () => { throw Object.assign(new Error('Denied'), { status: 403 }); };
  await assert.rejects(ensureRelease(args), /Denied/);
  console.log('Release creation, existing drafts/notes, concurrency and error handling passed.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
