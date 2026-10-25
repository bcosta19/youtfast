# Project website

The landing page is static HTML/CSS/JavaScript, in English and Brazilian
Portuguese. It has no package installation, external fonts, analytics, embeds
or API calls. All downloads link to the maintained GitHub Releases page.

## Preview

From the repository root:

```sh
python3 tools/build_site.py
python3 -m http.server 8765 --bind 127.0.0.1 --directory dist/site
```

Open `http://127.0.0.1:8765/`, or add `?lang=pt` for Portuguese. The language
switch works without storing anything; all content and links remain usable
in English without JavaScript. The demo MP4 loads only on user interaction.

The build copies an explicit allowlist of public assets into `dist/site/`.
It checks local references, section anchors, image alternatives, paired
translations and relative asset URLs. Relative URLs support both local
preview and the `/youtfast/` GitHub Pages project path.

## Publication

`.github/workflows/pages.yml` validates the site on pull requests and merge
queues. Only pushes to `main` affecting site inputs (or manual dispatch on
`main`) upload and deploy the Pages artifact. PRs have read-only permissions
and cannot publish. Deployment uses GitHub-owned, SHA-pinned Actions with
`pages: write` and `id-token: write` scoped to the deploy job.

The repository must have GitHub Pages enabled with **Source: GitHub Actions**
(`build_type: workflow` in the REST API). The `github-pages` environment must
allow deployments from `main` without a manual approval if publication should
happen automatically after merge. No personal token or additional secret is
needed in the workflow. Expected public address:

https://bcosta19.github.io/youtfast/

To update the hero poster, extract the application window from the existing
public demonstration (not a user's desktop or account data):

```sh
ffmpeg -ss 18 -i assets/demo/youtfast-demo.mp4 -frames:v 1 \
  -vf 'crop=1576:886:172:32,scale=1440:-2' -quality 86 website/assets/player.webp
```

The recording includes *Spring* by Blender Studio (CC BY 4.0); its credit and
link appear beneath the demo. Performance figures are the documented 0.1.0
search-page benchmark, not a playback or current-release measurement. Keep
package claims aligned with `docs/distribution.md` and the actual releases.
