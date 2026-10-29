# Project website

The landing page is static HTML/CSS/JavaScript, in English and Brazilian
Portuguese. It has no package installation, external fonts, analytics, embeds
or API calls. Download buttons link directly to the current release assets on GitHub; the build reads the version from `Cargo.toml` and fills the `{{VERSION}}` placeholder in `website/index.html`.

## Preview

From the repository root:

```sh
python3 tools/build_site.py
python3 -m http.server 8765 --bind 127.0.0.1 --directory dist/site
```

Open `http://127.0.0.1:8765/`, or open `/pt/` for Portuguese. The language
switch works without storing anything; all content and links remain usable
in both languages without JavaScript. The theme selector offers System, Light and
Dark. System follows `prefers-color-scheme`; a manual choice is stored only in
this browser's local storage. The demo MP4 loads only on user
interaction. The Cinema layout centers the introduction above a large product
image. Noto Sans is bundled locally as Latin and Latin-Extended WOFF2 subsets (made with `pyftsubset` from fonttools in a throwaway virtual environment), with its SIL Open Font License in
`assets/fonts/OFL.txt`; the page makes no font requests to external services.
Language navigation uses native cross-document View Transitions where supported.
Section navigation, FAQ answers and buttons have brief motion feedback. All
animations respect `prefers-reduced-motion`; links also work without animations.

The build copies an explicit allowlist of public assets into `dist/site/`.
It checks local references, section anchors, image alternatives, paired
translations and relative asset URLs. Relative URLs support both local
preview and static hosting. The build also renders the Portuguese page,
`robots.txt`, a bilingual sitemap and a 404 page.

## Publication

The production address is https://youtfast.pages.dev/ (English) and
https://youtfast.pages.dev/pt/ (Brazilian Portuguese). Publish only `dist/site`,
which contains the explicit public allowlist; previews in `dist/designs` are
never uploaded.

The download links point at the release named by `Cargo.toml`'s version. Deploy
only after that release is published, so the links never point at a release
that does not exist yet.

After authenticating with Wrangler, build and deploy:

```sh
python3 tools/build_site.py
npm exec --yes --package=wrangler@4.152.0 -- wrangler pages deploy dist/site --project-name youtfast --branch main
```

The Cloudflare project uses Direct Upload. These commands deploy an already
built site; they do not enable automatic Git-based deployment. The Website workflow validates PRs, merge queues and site changes on `main`;
it never deploys. The former GitHub Pages deployment has been removed.

## Search visibility

Both language pages contain visible product descriptions, canonical and
reciprocal hreflang links, Open Graph/Twitter metadata, and WebSite, WebPage
and SoftwareApplication JSON-LD. No ratings or reviews are invented.
`robots.txt` allows crawling and references the bilingual sitemap.
Search Console ownership verification and sitemap submission require the
site owner's Google account; they are not performed by the site build.
Google decides whether to index the pages and which snippets or AI summaries
to show. Metadata and structured data do not control that text verbatim.

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
