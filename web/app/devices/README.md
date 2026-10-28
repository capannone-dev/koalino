<!-- Documents the web Devices page route. -->

# Devices page

`page.tsx` serves `/devices` (Issue #43). It reads `GET /api/p2p/status` on the server and hands
it to `components/devices-panel.tsx`, which polls the status every two seconds and runs the
pairing steps: make this server discoverable, pair with a nearby device by comparing a six-digit
code, accept or reject another device's request, and remove a paired device.

When device sync is off the page says so; when the daemon cannot be reached it says that instead
of failing, so the page never needs the error boundary.
