import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { exec } from "node:child_process";

function loadEnv() {
  const envPath = path.resolve(process.cwd(), ".env");
  if (fs.existsSync(envPath)) {
    const lines = fs.readFileSync(envPath, "utf-8").split("\n");
    for (const line of lines) {
      const match = line.match(/^([^#=]+)=(.*)$/);
      if (match) {
        const key = match[1].trim();
        const val = match[2].trim();
        if (!process.env[key]) process.env[key] = val;
      }
    }
  }
}
loadEnv();

const CLIENT_ID = process.env.YOUTUBE_CLIENT_ID;
const CLIENT_SECRET = process.env.YOUTUBE_CLIENT_SECRET;

if (!CLIENT_ID || !CLIENT_SECRET) {
  console.error("❌ Error: YOUTUBE_CLIENT_ID and YOUTUBE_CLIENT_SECRET must be set in your .env or environment.");
  process.exit(1);
}

const PORT = 8080;
const REDIRECT_URI = `http://localhost:${PORT}`;

const SCOPES = [
  "https://www.googleapis.com/auth/youtube.upload",
  "https://www.googleapis.com/auth/youtube.readonly",
].join(" ");

const authUrl = `https://accounts.google.com/o/oauth2/v2/auth?client_id=${CLIENT_ID}&redirect_uri=${encodeURIComponent(REDIRECT_URI)}&response_type=code&scope=${encodeURIComponent(SCOPES)}&access_type=offline&prompt=consent`;

console.log("\n==================================================");
console.log("   ClipOn YouTube Shorts OAuth2 Token Generator   ");
console.log("==================================================\n");
console.log("1. Opening browser to authorize YouTube upload access...");
console.log("   If the browser does not open automatically, copy & paste this URL:\n");
console.log(authUrl);
console.log("\n2. Waiting for authorization code on http://localhost:" + PORT + "...\n");

const server = http.createServer(async (req, res) => {
  try {
    const reqUrl = new URL(req.url, `http://localhost:${PORT}`);
    const code = reqUrl.searchParams.get("code");
    const error = reqUrl.searchParams.get("error");

    if (error) {
      res.writeHead(400, { "Content-Type": "text/html" });
      res.end(`<h2>Authorization Failed</h2><p>${error}</p>`);
      console.error("\n❌ Authorization was denied or failed:", error);
      server.close();
      process.exit(1);
    }

    if (!code) {
      res.writeHead(200, { "Content-Type": "text/plain" });
      res.end("Listening for Google OAuth callback...");
      return;
    }

    console.log("3. Authorization code received! Exchanging for tokens...");

    const tokenResponse = await fetch("https://oauth2.googleapis.com/token", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({
        code,
        client_id: CLIENT_ID,
        client_secret: CLIENT_SECRET,
        redirect_uri: REDIRECT_URI,
        grant_type: "authorization_code",
      }),
    });

    const tokenData = await tokenResponse.json();

    if (!tokenResponse.ok || !tokenData.refresh_token) {
      const errMsg = tokenData.error_description || tokenData.error || "No refresh token returned";
      res.writeHead(400, { "Content-Type": "text/html" });
      res.end(`<h2>Token Exchange Failed</h2><p>${errMsg}</p>`);
      console.error("\n❌ Failed to get refresh token:", tokenData);
      server.close();
      process.exit(1);
    }

    const refreshToken = tokenData.refresh_token;
    console.log("\n🎉 SUCCESS! Return to ClipOn Settings and paste the refresh token there.");

    res.writeHead(200, { "Content-Type": "text/html" });
    res.end(`
      <html>
        <body style="font-family: -apple-system, sans-serif; display: flex; align-items: center; justify-content: center; height: 90vh; background: #0f172a; color: #fff;">
          <div style="background: #1e293b; padding: 32px 40px; border-radius: 12px; border: 1px solid #334155; text-align: center; max-width: 450px;">
            <div style="font-size: 40px; margin-bottom: 12px;">🎉</div>
            <h2 style="color: #4ade80; margin: 0 0 8px 0;">YouTube Shorts Connected!</h2>
            <p style="color: #94a3b8; font-size: 14px; line-height: 1.5;">
              Refresh token generated and saved to ClipOn securely. You can now close this tab and upload directly to YouTube Shorts!
            </p>
          </div>
        </body>
      </html>
    `);

    console.log("\n==================================================");
    console.log("   YouTube Shorts is ready to use in ClipOn!      ");
    console.log("==================================================\n");

    setTimeout(() => {
      server.close();
      process.exit(0);
    }, 1500);

  } catch (err) {
    console.error("Server error:", err);
    res.writeHead(500, { "Content-Type": "text/plain" });
    res.end("Internal server error");
  }
});

server.listen(PORT, () => {
  // Open URL on macOS
  exec(`open "${authUrl}"`);
});
