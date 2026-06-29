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

## Social Platform

The social backend server (`server/`) provides real-time social features:

- **Feed** with @mentions and #hashtags
- **Minecraft identity** — profile locked to Minecraft username + skin, only badge is editable
- **Server sharing** — share AMT servers or custom IP servers
- **Badge & Cape showcase** — share cosmetics in posts
- **Recommendation ranking** — feed sorted by likes + recency
- **Trending hashtags** — discover popular topics

### Deploy the Social Server

**Option 1: Docker** (recommended)
```bash
docker build -t amt-social-server server/
docker run -d -p 8080:8080 -v amt_social_data:/data amt-social-server
```

**Option 2: Railway / Fly.io**
```bash
# Deploy the server/ directory directly
# Set BIND=0.0.0.0:8080 and DB_PATH=/data/amt_social.db
```

**Option 3: Bare metal**
```bash
cd server
cargo run --release
# Binds to 0.0.0.0:8080 by default, set BIND=0.0.0.0:3000 to change
```

> **Configure in launcher:** Settings → General → set `Social API URL` to your deployed server URL

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/social/register` | Register/update user (uuid, mc_username, badge) |
| GET | `/api/social/users/:uuid` | Get user profile |
| POST | `/api/social/posts` | Create a post (supports @mentions, #hashtags, attachments) |
| GET | `/api/social/feed` | Get global feed (query: tag, user, search, limit, offset) |
| GET | `/api/social/posts/:id` | Get single post |
| POST | `/api/social/posts/:id/like` | Like/unlike a post |
| GET | `/api/social/hashtags` | Trending hashtags (last 7 days) |

### Post Types

| Type | Description | attachment_data |
|------|-------------|-----------------|
| `text` | Plain text with @mentions and #hashtags | null |
| `server_invite` | Server share | `{ server_id, server_name, server_type, mc_version, address, player_count, max_players }` |
| `badge_showcase` | Badge share | `{ badge, minecraft_username }` |
| `cosmetics_share` | Cape showcase | `{ cape_id, badge_text }` |

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
