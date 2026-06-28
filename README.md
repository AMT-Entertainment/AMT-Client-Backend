# AMT Backend

GitHub-hosted zero-cost backend for the AMT Client.

## Architecture

All data is served via `raw.githubusercontent.com` (free CDN, globally cached). Writes use GitHub Actions via `repository_dispatch`.

```
raw.githubusercontent.com/OWNER/REPO/main/
├── api/v1/version/branches.json   → Available branches
├── api/v1/version/builds/         → Build listings per branch
├── api/v1/version/launch/         → Launch manifests per build
├── api/v1/version/mods/           → Mod listings per MC version
├── api/v1/version/changelog/      → Build changelogs
├── api/v3/blog/                   → Blog/news posts
├── data/gallery.json              → Auto-generated cape gallery index
├── data/capes/*.json              → Cape metadata
├── data/users/*.json              → User badge/profile data
└── assets/capes/*.png             → Cape textures
```

## Quick Start

1. Fork this repo to your GitHub account
2. Update the `api/v1/` directory with your client builds
3. Configure the launcher at **Settings → General**:
   - **Backend URL**: `https://raw.githubusercontent.com/YOUR_USER/amt-backend/main`
   - **GitHub Repo Owner**: `YOUR_USER`
   - **GitHub Repo Name**: `amt-backend`

See [BACKEND_SETUP.md](../BACKEND_SETUP.md) for the full guide.

## Workflows

| Workflow | Trigger | Description |
|----------|---------|-------------|
| `upload-cape.yml` | `repository_dispatch` (upload-cape) | Validate and commit new cape PNG + metadata |
| `update-badge.yml` | `repository_dispatch` (update-badge) | Update user badge and display name |
| `vote.yml` | `repository_dispatch` (vote-cape) | Increment/decrement cape vote count |
| `update-gallery.yml` | Push to `data/capes/`, manual | Rebuild `data/gallery.json` index |

## Manual API Calls

**Read** (no auth required):
```
GET https://raw.githubusercontent.com/OWNER/REPO/main/api/v1/version/branches.json
GET https://raw.githubusercontent.com/OWNER/REPO/main/data/gallery.json
```

**Write** (GitHub token with `repo` scope):
```
POST https://api.github.com/repos/OWNER/REPO/dispatches
Authorization: Bearer {token}
{
  "event_type": "upload-cape",
  "client_payload": { ... }
}
```

## License

Same as the AMT Client — GPL-3.0.
