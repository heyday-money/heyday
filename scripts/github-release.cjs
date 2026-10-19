// Shared by platform workflows. Preserve existing release notes and draft status.
module.exports = async function ensureRelease({ github, context, tag, prerelease }) {
  const find = async () => {
    for await (const { data } of github.paginate.iterator(
      github.rest.repos.listReleases, { ...context.repo, per_page: 100 },
    )) {
      const release = data.find(candidate => candidate.tag_name === tag);
      if (release) return release;
    }
  };
  const existing = await find();
  if (existing) return existing;
  try {
    const { data } = await github.rest.repos.createRelease({
      ...context.repo, tag_name: tag, name: `Heyday Money ${tag}`,
      body: 'Download the installer for your operating system below. Platform assets appear as their builds finish.\nmacOS builds are ad-hoc signed and not notarized. Windows installers are unsigned.',
      draft: false, prerelease,
    });
    return data;
  } catch (error) {
    // Another platform can create this tag release while our build is running.
    if (error.status !== 422) throw error;
    for (let attempt = 0; attempt < 5; attempt++) {
      const release = await find();
      if (release) return release;
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    throw error;
  }
};
