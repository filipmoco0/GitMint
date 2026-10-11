# GitHub sign-in

GitMint requires GitHub sign-in to browse repositories. The beta uses a public OAuth client ID and an HTTPS token broker. The client ID is not a secret; the OAuth client secret stays in the broker's deployment secrets and must never be added to the desktop app or `.env.local`.

## Use the GitMint beta configuration

Copy `.env.example` to `.env.local` and keep its values to use the GitMint beta OAuth app and hosted broker. The broker exchanges short-lived authorization codes and does not store or log access tokens. GitMint stores the returned token in macOS Keychain.

## Run your own OAuth app and broker

Register a GitHub OAuth App with callback URL `http://127.0.0.1/callback`. Disable wildcard matching and leave Device Flow disabled. Put the OAuth client ID in `VITE_GITHUB_CLIENT_ID`; never put the client secret in the desktop app.

Deploy the worker in `auth-broker/` and provide the secret at deploy time:

```sh
cd auth-broker
npx wrangler secret put GITHUB_CLIENT_SECRET
npx wrangler deploy
```

Set `VITE_GITMINT_AUTH_BROKER_URL` to the deployed HTTPS worker URL. Update `GITHUB_CLIENT_ID` in `wrangler.toml` to match your OAuth app. Do not commit worker secrets or `.env.local`.

The desktop app uses PKCE, a random state value, and a temporary loopback listener for sign-in. It requests GitHub's `read:user` and `repo` scopes. GitHub's classic `repo` scope allows write access to private repositories; GitMint currently uses the API to list repositories and display their links. Local repository changes use the Git CLI.
