import { useEffect, useState } from 'react';
import QRCode from 'qrcode';
import { getDevicePairing, rotateDeviceToken, unpairDevice } from '../api/drivers';
import { serverAddress } from '../api/driverApp';
import { getRole } from '../api/auth';
import { pairingLink, timeAgo } from '../lib/pairingLink';
import { IconCheck } from './Icons';
import type { DevicePairing } from '../types';

/**
 * Pair a driver's phone with the Android driver app: issue a device token and
 * show it once, as a QR code the app scans (with the server address in it) and
 * as text to type. Also shows whether a phone is paired and when it last
 * reported, and unpairs a lost phone. The server keeps only a hash of the
 * token, so it can't be shown again: pairing again issues a new one.
 */
export default function PhonePairingCard({ driverId }: { driverId: string }) {
  const [pairing, setPairing] = useState<DevicePairing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<'repair' | 'unpair' | null>(null);
  // The token just issued; only in memory, and only until "Done".
  const [issued, setIssued] = useState<string | null>(null);
  // The QR image with the token it encodes, so an old one is never shown
  // for a newer token.
  const [qr, setQr] = useState<{ token: string; image: string } | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const canManage = getRole() !== 'WAREHOUSE_STAFF';
  const server = serverAddress();

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await getDevicePairing(driverId);
        if (!cancelled) setPairing(res.data ?? null);
      } catch {
        if (!cancelled) setError("Couldn't load the phone's pairing.");
      }
    })();
    return () => { cancelled = true; };
  }, [driverId]);

  useEffect(() => {
    if (!issued) return;
    let cancelled = false;
    QRCode.toDataURL(pairingLink(server, issued), { margin: 1, width: 220, color: { dark: '#111111', light: '#ffffff' } })
      .then(image => { if (!cancelled) setQr({ token: issued, image }); })
      .catch(() => { if (!cancelled) setQr(null); });
    return () => { cancelled = true; };
  }, [issued, server]);

  const run = async (action: () => Promise<void>, failure: string) => {
    setBusy(true);
    setError(null);
    setConfirm(null);
    try {
      await action();
    } catch {
      setError(failure);
    } finally {
      setBusy(false);
    }
  };

  const pair = () => run(async () => {
    const res = await rotateDeviceToken(driverId);
    setIssued(res.data?.device_token ?? null);
    setPairing({ paired: true, paired_at: Math.floor(Date.now() / 1000), last_seen_at: null });
  }, "Couldn't issue a device token.");

  const unpair = () => run(async () => {
    const res = await unpairDevice(driverId);
    setIssued(null);
    setPairing(res.data ?? { paired: false, paired_at: null, last_seen_at: null });
  }, "Couldn't unpair the phone.");

  const copy = async (what: string, text: string) => {
    await navigator.clipboard.writeText(text);
    setCopied(what);
    setTimeout(() => setCopied(null), 2000);
  };

  const paired = pairing?.paired ?? false;

  return (
    <div className="section-card" style={{ maxWidth: 460, marginTop: 20 }} data-testid="phone-pairing-card">
      <div className="section-card-header">
        <span className="section-card-title">Driver's phone</span>
        {pairing && (
          <span className={paired ? 'badge tag-green' : 'badge'} data-testid="pairing-status">
            {paired ? 'Paired' : 'Not paired'}
          </span>
        )}
      </div>
      <div className="section-card-body" style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        {error && <p className="errortxt" style={{ margin: 0 }}>{error}</p>}

        {pairing && !issued && (
          <p className="muted" style={{ margin: 0 }} data-testid="pairing-detail">
            {!paired && 'No phone is paired. Pair one so this driver\'s vehicle follows their phone on the map.'}
            {paired && (pairing.paired_at ? `Paired ${timeAgo(pairing.paired_at)}` : 'Paired')}
            {paired && ' · '}
            {paired && (pairing.last_seen_at
              ? `last reported ${timeAgo(pairing.last_seen_at)}`
              : "the phone hasn't connected yet")}
          </p>
        )}

        {issued && (
          <div data-testid="issued-token" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <p style={{ margin: 0, fontSize: 13 }}>
              In the Logistics Driver app, tap <strong>Scan QR code</strong> on the pairing screen and scan this,
              then tap <strong>Pair</strong>. Or type the server address and token.
            </p>
            {qr?.token === issued && (
              <img src={qr.image} alt="Pairing QR code" width={180} height={180} data-testid="pairing-qr"
                style={{ borderRadius: 8, border: '1px solid var(--border)' }} />
            )}
            <div>
              <div className="muted">Server address</div>
              <code data-testid="pairing-server" style={{ fontSize: 13 }}>{server}</code>
            </div>
            <div>
              <div className="muted">Device token</div>
              <code data-testid="pairing-token" style={{ fontSize: 13, wordBreak: 'break-all' }}>{issued}</code>
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => copy('token', issued)}>
                {copied === 'token' ? <><IconCheck size={12} /> Copied</> : 'Copy token'}
              </button>
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => copy('server', server)}>
                {copied === 'server' ? <><IconCheck size={12} /> Copied</> : 'Copy server address'}
              </button>
            </div>
            <p className="muted" style={{ margin: 0, fontSize: 12 }}>
              Shown only this once. Anyone with the token can report this driver's position, so hand it only to
              the driver. Pairing again or unpairing makes it stop working.
            </p>
            <div>
              <button type="button" className="btn btn-primary btn-sm" onClick={() => setIssued(null)}>Done</button>
            </div>
          </div>
        )}

        {pairing && !issued && canManage && (
          confirm ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }} data-testid="pairing-confirm">
              <p style={{ margin: 0, fontSize: 13 }}>
                {confirm === 'repair'
                  ? 'Pair a new phone? The phone paired now stops reporting at once.'
                  : 'Unpair the phone? It stops reporting at once, until a phone is paired again.'}
              </p>
              <div style={{ display: 'flex', gap: 8 }}>
                <button type="button" className="btn btn-primary btn-sm" disabled={busy}
                  onClick={confirm === 'repair' ? pair : unpair}>
                  {confirm === 'repair' ? 'Pair a new phone' : 'Unpair'}
                </button>
                <button type="button" className="btn btn-ghost btn-sm" onClick={() => setConfirm(null)}>Cancel</button>
              </div>
            </div>
          ) : (
            <div style={{ display: 'flex', gap: 8 }}>
              {!paired && (
                <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={pair}>
                  Pair a phone
                </button>
              )}
              {paired && (
                <>
                  <button type="button" className="btn btn-ghost btn-sm" disabled={busy} onClick={() => setConfirm('repair')}>
                    Pair a new phone
                  </button>
                  <button type="button" className="btn btn-ghost btn-sm" disabled={busy} onClick={() => setConfirm('unpair')}>
                    Unpair
                  </button>
                </>
              )}
            </div>
          )
        )}
        {pairing && !canManage && (
          <p className="muted" style={{ margin: 0, fontSize: 12 }}>Only an Admin or Dispatcher can pair or unpair phones.</p>
        )}
      </div>
    </div>
  );
}
