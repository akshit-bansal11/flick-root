# Media controls

Control whatever is playing, and search Spotify or YouTube, after typing `>`. Everything works without an account; connecting Spotify is optional.

## How to use it (no setup)

| Type | Result |
| --- | --- |
| `play`, `pause`, `next`, `previous` (or `prev`), `stop` | Sends the command to the current Windows media session, so it works with any player that registers one. |
| `now playing` or `np` | Shows the current track. |
| `play <query> on spotify` | Opens Spotify's search for the query in the Spotify app. |
| `play <query> on youtube` | Opens the YouTube results page in your browser. |

The media session is read only when you press Enter on a row; no task runs in the background.

## YouTube API key (optional)

Settings, Integrations, YouTube: add your own YouTube Data API key and `play X on youtube` opens the first result instead of the search page. If the lookup fails it falls back to the search page. The key is stored as `integration.youtube@www.googleapis.com` and sent only to `www.googleapis.com`.

## Connecting Spotify (optional)

Once connected:

| Type | Result |
| --- | --- |
| `play <query> on spotify` | Plays the top track (Premium). Without Premium, or with no active device, it opens the track in the Spotify app instead. |
| `queue <query>` | Adds the top track to the queue (Premium). |
| `like` | Saves the current track to your library. |
| `devices` | Lists your devices; pick one to move playback (Premium). |
| `playlist <query>` | Searches your first 50 playlists; pick one to play it (Premium). |

The rows say "(Premium)" where Spotify requires it.

### How to make your own Spotify developer app

Flick has no shared Spotify app (Spotify's development-mode limits make a shared one unworkable), so you make your own. These steps follow https://developer.spotify.com/documentation/web-api/concepts/apps :

1. Open your Spotify developer Dashboard and click **Create an App**.
2. Enter an app name and a description, tick the Developer Terms of Service box, and click Create.
3. Under Edit Settings, add this exact redirect URI: `http://127.0.0.1:43919/callback`. Spotify requires an exact match, allows http only for loopback addresses, and does not allow `localhost` (https://developer.spotify.com/documentation/web-api/concepts/redirect_uri). The Settings page shows the URI to use. Port 43919 is the default; it can be changed with `redirect_port` under `[integrations.spotify]` in the config, and then the URI changes with it.
4. Copy the app's Client ID, paste it in Settings, Integrations, Spotify, and click **Connect Spotify**. Your browser opens Spotify's consent page; Flick waits up to 3 minutes for you to come back.

Flick uses OAuth PKCE, so there is no client secret to store. It asks for these scopes only: `user-modify-playback-state`, `user-read-playback-state`, `user-read-currently-playing`, `user-library-modify`, `playlist-read-private`.

New apps start in Development Mode, which limits the number of users and API requests. Per Spotify's February 2026 migration guide (https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide), Development Mode apps created from 11 February 2026 allow 5 users per app, and every Development Mode app requires the **app owner** to have an active Spotify Premium subscription; the app stops working if that lapses. The same guide says the limit of Client IDs per developer was raised to 25 as of July 2026. Spotify can change these rules; check the current docs.

### Privacy

- Off until you connect. Nothing talks to Spotify before that.
- Connected, Flick talks to `accounts.spotify.com` and `api.spotify.com` with your own Client ID, from the Rust side only (never the WebView), and never on the keystroke path.
- Only a refresh token is kept, in Windows Credential Manager as `integration.spotify.refresh@accounts.spotify.com`. Access tokens stay in memory. **Disconnect Spotify** removes the connection.
- The callback listener runs only while you are connecting.
- If Spotify revokes the token, or you change the Client ID, Flick disconnects automatically; connect again.

## Limits

- Play, pause and the other controls only work with players that register a Windows media session.
- `playlist` searches only your first 50 playlists.
- Without Premium, Spotify playback control is not available; Flick falls back to opening the Spotify app.
