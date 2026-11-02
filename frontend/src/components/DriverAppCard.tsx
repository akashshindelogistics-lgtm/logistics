import { useEffect, useState } from 'react';
import QRCode from 'qrcode';
import { downloadUrl, getDriverAppRelease, serverAddress } from '../api/driverApp';
import { IconCheck } from './Icons';
import type { DriverAppRelease } from '../types';

/**
 * What the office needs to get a driver's phone set up: where to download
 * the Android app (a link and a QR code the driver scans with the phone's
 * camera), and the server address to enter when pairing. The link is the
 * same for every release, and the app itself offers newer releases.
 */
export default function DriverAppCard() {
  // undefined: loading; null: nothing published.
  const [release, setRelease] = useState<DriverAppRelease | null | undefined>(undefined);
  const [failed, setFailed] = useState(false);
  const [qr, setQr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const server = serverAddress();

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const r = await getDriverAppRelease();
        if (!cancelled) setRelease(r);
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const url = release ? downloadUrl(release, server) : null;

  useEffect(() => {
    if (!url) return;
    let cancelled = false;
    QRCode.toDataURL(url, { margin: 1, width: 168, color: { dark: '#111111', light: '#ffffff' } })
      .then(data => { if (!cancelled) setQr(data); })
      .catch(() => { if (!cancelled) setQr(null); });
    return () => { cancelled = true; };
  }, [url]);

  const copy = async () => {
    if (!url) return;
    await navigator.clipboard.writeText(url);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="section-card" style={{ maxWidth: 460, marginTop: 20 }} data-testid="driver-app-card">
      <div className="section-card-header">
        <span className="section-card-title">Driver app (Android)</span>
        {release && <span className="badge" data-testid="driver-app-version">v{release.version_name}</span>}
      </div>
      <div className="section-card-body">
        {failed && <p className="errortxt">Couldn't check for a published driver app.</p>}
        {release === undefined && !failed && <p className="muted">Checking for the driver app…</p>}
        {release === null && (
          <p className="muted" data-testid="driver-app-none">
            No driver app has been published on this server yet. Build and publish one with
            <code> android/scripts/build-release.sh</code> (see the driver install guide).
          </p>
        )}
        {release && url && (
          <div style={{ display: 'flex', gap: 18, alignItems: 'flex-start' }}>
            {qr && (
              <img
                src={qr}
                alt={`QR code for ${url}`}
                width={140}
                height={140}
                style={{ borderRadius: 8, border: '1px solid var(--border)', flexShrink: 0 }}
                data-testid="driver-app-qr"
              />
            )}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10, minWidth: 0 }}>
              <p className="muted" style={{ margin: 0 }}>
                The driver scans the code with the phone's camera, or opens the link, and installs the app.
                Then they pair it with the server address below and a device token.
              </p>
              <div>
                <div className="muted">Download link</div>
                <a href={url} data-testid="driver-app-link" style={{ wordBreak: 'break-all', fontSize: 13 }}>{url}</a>
              </div>
              <div>
                <div className="muted">Server address</div>
                <code data-testid="driver-app-server" style={{ fontSize: 13 }}>{server}</code>
              </div>
              <div className="muted" style={{ fontSize: 12 }}>
                {(release.size_bytes / 1_000_000).toFixed(1)} MB · released {new Date(release.released_at * 1000).toLocaleDateString()}
              </div>
              <div>
                <button type="button" className="btn btn-ghost btn-sm" onClick={copy}>
                  {copied ? <><IconCheck size={12} /> Copied</> : 'Copy link'}
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
