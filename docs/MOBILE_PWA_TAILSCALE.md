# Use Mint Web on a phone with Tailscale and HTTPS

This guide shows how to open Mint Agent on a phone and add its Web UI to the home screen as a PWA. Mint and its AI backend continue running on the computer. The phone connects to that computer through your private Tailscale network (tailnet).

`mint web --tailscale` is optional. Users of the standard `mint web`, CLI, and desktop app do not need Tailscale.

## 1. Install Tailscale on the computer

On Linux, including Pop!_OS and Ubuntu, run the commands from the [Tailscale Linux installation guide](https://tailscale.com/docs/install/linux):

```bash
curl -fsSL https://tailscale.com/install.sh | sh
sudo tailscale up
```

The second command prints a sign-in URL. Open it in a browser and sign in. Then allow your regular user account to manage Tailscale Serve so Mint does not need to run as root:

```bash
sudo tailscale set --operator="$(whoami)"
tailscale status
```

See Tailscale's [Linux operator permissions guide](https://tailscale.com/docs/reference/troubleshooting/linux/linux-operator-permission) for details. On other operating systems, install Tailscale from its [download page](https://tailscale.com/download) and sign in.

## 2. Install Tailscale on the phone

- **Android:** Install Tailscale from Google Play using the [Android guide](https://tailscale.com/docs/install/android).
- **iPhone:** Install Tailscale from the App Store using the [iOS guide](https://tailscale.com/docs/install/ios).

Sign in with the same account used on the computer, approve the VPN configuration requested by the phone, and connect. Both devices must be in the same tailnet.

## 3. Enable MagicDNS and HTTPS

Open the [Tailscale DNS settings](https://login.tailscale.com/admin/dns) and enable **MagicDNS** and **HTTPS Certificates** as described in the [HTTPS guide](https://tailscale.com/docs/how-to/set-up-https-certificates). The machine and tailnet names in an HTTPS certificate appear in the public Certificate Transparency log, so choose a machine name that does not reveal private information.

## 4. Build and start Mint Web

From the Mint project directory, run:

```bash
npm run build:web
mint web --tailscale
```

If you are running Mint from source and have not installed the `mint` command, replace the second line with `cargo run -p mint-cli -- web --tailscale`. Stop any existing `mint web` process first: this mode needs local ports `9000` and `3000`, and Tailscale Serve needs HTTPS port `443` available.

When Tailscale Serve prints **Available within your tailnet**, open the **Mobile HTTPS** URL on your phone, for example `https://mint-computer.example.ts.net/`. The `Mobile: http://...:9000` address shown by standard `mint web` is for its LAN mode. The `127.0.0.1:9000` address works only on the computer running Mint.

## 5. Add Mint to the home screen

With Tailscale connected on the phone, open the HTTPS URL in a browser:

- **Android/Chrome:** Choose **Install app** or **Add to Home screen** from the browser menu.
- **iPhone/Safari:** Tap **Share**, then **Add to Home Screen**.

You can then open Mint from its home-screen icon. The computer must remain awake and `mint web --tailscale` must keep running. Chat and agent features require the backend on the computer and do not work offline.

## Stop Mint and troubleshoot

- **Stop Mint Web:** Press **Ctrl+C** in the terminal running `mint web --tailscale`. This stops Mint Web and the Serve session started by that command. Tailscale itself stays connected on both devices until you disconnect it in the Tailscale app.
- **The phone cannot open the URL:** Check that both devices are connected to the same tailnet, the computer is awake, Mint is still running, and Serve shows **Available within your tailnet**.
- **Port `9000` or `3000` is in use:** Stop the existing `mint web` process or other service using that port, then restart tailnet mode.
- **Serve reports that port `443` is in use:** Inspect `tailscale serve status`. Mint will not replace an existing Serve route on that port.
- **Linux reports a permission error:** Confirm that your account is the Tailscale operator, as shown in step 1, and that Tailscale sign-in succeeded.

This setup uses **Tailscale Serve** inside your tailnet. It does not require forwarding ports `9000` or `3000` on your router. Do not use **Tailscale Funnel** for this setup; Funnel publishes the service to the public internet.
