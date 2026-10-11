const json = (body, status = 200) => new Response(JSON.stringify(body), {
  status,
  headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store" },
});

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method !== "POST" || url.pathname !== "/token") return json({ error: "not_found" }, 404);
    if (!env.GITHUB_CLIENT_ID || !env.GITHUB_CLIENT_SECRET) return json({ error: "not_configured" }, 503);

    let body;
    try { body = await request.json(); }
    catch { return json({ error: "invalid_request" }, 400); }

    if (body.client_id !== env.GITHUB_CLIENT_ID || typeof body.code !== "string" || body.code.length > 2048 ||
        typeof body.code_verifier !== "string" || !/^[A-Za-z0-9._~-]{43,128}$/.test(body.code_verifier) ||
        typeof body.redirect_uri !== "string") return json({ error: "invalid_request" }, 400);

    let redirect;
    try { redirect = new URL(body.redirect_uri); }
    catch { return json({ error: "invalid_redirect" }, 400); }
    const port = Number(redirect.port);
    if (redirect.protocol !== "http:" || redirect.hostname !== "127.0.0.1" || redirect.pathname !== "/callback" ||
        !Number.isInteger(port) || port < 1 || port > 65535) return json({ error: "invalid_redirect" }, 400);

    const githubResponse = await fetch("https://github.com/login/oauth/access_token", {
      method: "POST",
      headers: { accept: "application/json", "content-type": "application/json", "user-agent": "GitMint" },
      body: JSON.stringify({
        client_id: env.GITHUB_CLIENT_ID,
        client_secret: env.GITHUB_CLIENT_SECRET,
        code: body.code,
        redirect_uri: body.redirect_uri,
        code_verifier: body.code_verifier,
      }),
    });

    const token = await githubResponse.json().catch(() => null);
    if (!githubResponse.ok || !token?.access_token) return json({ error: "github_exchange_failed" }, 401);
    return json({ access_token: token.access_token });
  },
};
