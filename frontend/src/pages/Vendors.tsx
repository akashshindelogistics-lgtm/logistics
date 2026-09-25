import { useEffect, useState } from 'react';
import { listVendors, createVendor, updateVendor, deleteVendor } from '../api/vendors';
import { getOrgId, getRole } from '../api/auth';
import { IconTruck, IconPlus, IconX, IconTrash } from '../components/Icons';
import type { VehicleVendor, VendorInput } from '../types';
import './page.css';

const emptyForm = { name: '', contact_person: '', phone: '', gstin: '', notes: '' };
type FormState = typeof emptyForm;

/** Blank optional fields go to the API as `null`, not an empty string. */
const toInput = (f: FormState): VendorInput => ({
  name: f.name,
  contact_person: f.contact_person.trim() || null,
  phone: f.phone,
  gstin: f.gstin.trim() || null,
  notes: f.notes.trim() || null,
});

const toForm = (v: VehicleVendor): FormState => ({
  name: v.name,
  contact_person: v.contact_person ?? '',
  phone: v.phone,
  gstin: v.gstin ?? '',
  notes: v.notes ?? '',
});

export default function Vendors() {
  const [vendors, setVendors] = useState<VehicleVendor[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  // The vendor being edited, or null when the form adds a new one.
  const [editing, setEditing] = useState<VehicleVendor | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');

  // Every role can see vendors; only Admin and Dispatcher can change them.
  const canEdit = getRole() !== 'WAREHOUSE_STAFF';

  const load = () => {
    const orgId = getOrgId();
    if (!orgId) { setLoading(false); return; }
    listVendors(orgId).then(r => setVendors(r.data ?? [])).catch(() => { /* ignore */ }).finally(() => setLoading(false));
  };
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const set = (field: keyof FormState) => (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) =>
    setForm({ ...form, [field]: e.target.value });

  const closeForm = () => {
    setShowForm(false);
    setEditing(null);
    setForm(emptyForm);
    setError('');
  };

  const openAdd = () => {
    if (showForm) { closeForm(); return; }
    setEditing(null);
    setForm(emptyForm);
    setShowForm(true);
  };

  const openEdit = (v: VehicleVendor) => {
    setEditing(v);
    setForm(toForm(v));
    setError('');
    setShowForm(true);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const orgId = getOrgId();
    if (!orgId) return;
    setSubmitting(true);
    setError('');
    try {
      if (editing) {
        await updateVendor(editing.id, { ...toInput(form), is_active: editing.is_active });
      } else {
        await createVendor(orgId, toInput(form));
      }
      closeForm();
      load();
    } catch (err) {
      const msg = (err as { response?: { data?: { message?: string } } }).response?.data?.message;
      setError(msg || 'Could not save the vendor.');
    } finally {
      setSubmitting(false);
    }
  };

  const toggleActive = async (v: VehicleVendor) => {
    await updateVendor(v.id, {
      name: v.name, contact_person: v.contact_person, phone: v.phone,
      gstin: v.gstin, notes: v.notes, is_active: !v.is_active,
    });
    load();
  };

  const remove = async (v: VehicleVendor) => {
    if (!window.confirm(`Delete vendor ${v.name}?`)) return;
    await deleteVendor(v.id);
    load();
  };

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title-group">
          <h1>Vehicle Vendors</h1>
          <p>Transporters and brokers you hire trucks from when your own fleet can't take a dispatch</p>
        </div>
        {canEdit && (
          <button className="btn btn-primary" onClick={openAdd}>
            {showForm ? <><IconX size={14} />Cancel</> : <><IconPlus size={14} />Add Vendor</>}
          </button>
        )}
      </div>

      {showForm && (
        <div className="form-panel">
          <h2>{editing ? `Edit ${editing.name}` : 'Add Vendor'}</h2>
          <form onSubmit={handleSubmit}>
            <div className="field">
              <label htmlFor="vendor-name">Vendor name</label>
              <input id="vendor-name" placeholder="e.g. Sharma Roadlines" value={form.name} onChange={set('name')} required />
            </div>
            <div className="field">
              <label htmlFor="vendor-contact">Contact person</label>
              <input id="vendor-contact" placeholder="Optional" value={form.contact_person} onChange={set('contact_person')} />
            </div>
            <div className="field">
              <label htmlFor="vendor-phone">Phone</label>
              <input id="vendor-phone" placeholder="+91 98200 00000" value={form.phone} onChange={set('phone')} required />
            </div>
            <div className="field">
              <label htmlFor="vendor-gstin">GSTIN</label>
              <input id="vendor-gstin" placeholder="Optional, 15 characters" maxLength={15} value={form.gstin} onChange={set('gstin')} />
            </div>
            <div className="field">
              <label htmlFor="vendor-notes">Notes</label>
              <textarea id="vendor-notes" rows={2} placeholder="Rate terms, lanes served, truck types…" value={form.notes} onChange={set('notes')} />
            </div>
            {error && <div className="errortxt" style={{ marginBottom: 12 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn btn-primary" type="submit" disabled={submitting}>
                {submitting ? 'Saving…' : editing ? 'Save Vendor' : 'Add Vendor'}
              </button>
              <button className="btn btn-ghost" type="button" onClick={closeForm}>Cancel</button>
            </div>
          </form>
        </div>
      )}

      <div className="table-card">
        <div className="table-toolbar">
          <span className="table-toolbar-title">Vendors</span>
          {!loading && <span className="badge">{vendors.length}</span>}
        </div>
        {loading ? (
          <div style={{ padding: 20 }}><div className="skeleton" style={{ height: 20 }} /></div>
        ) : vendors.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon"><IconTruck size={26} /></div>
            <h3>No vendors yet</h3>
            <p>Add the transporters you call for trucks, so a dispatch can go out on a hired vehicle.</p>
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr><th>Vendor</th><th>Contact</th><th>Phone</th><th>GSTIN</th><th>Notes</th><th>Status</th>{canEdit && <th></th>}</tr>
              </thead>
              <tbody>
                {vendors.map(v => (
                  <tr key={v.id}>
                    <td className="entity-name">{v.name}</td>
                    <td className="muted">{v.contact_person ?? '—'}</td>
                    <td>{v.phone}</td>
                    <td className="muted mono">{v.gstin ?? '—'}</td>
                    <td className="muted">{v.notes ?? '—'}</td>
                    <td>
                      {canEdit ? (
                        <button
                          className={`badge ${v.is_active ? 'tag-green' : ''}`}
                          onClick={() => toggleActive(v)}
                          style={{ cursor: 'pointer', border: 'none' }}
                          aria-label={`${v.is_active ? 'Deactivate' : 'Activate'} ${v.name}`}
                        >
                          {v.is_active ? 'Active' : 'Inactive'}
                        </button>
                      ) : (
                        <span className={`badge ${v.is_active ? 'tag-green' : ''}`}>{v.is_active ? 'Active' : 'Inactive'}</span>
                      )}
                    </td>
                    {canEdit && (
                      <td style={{ textAlign: 'right', whiteSpace: 'nowrap' }}>
                        <button className="btn btn-ghost btn-sm" onClick={() => openEdit(v)} aria-label={`Edit ${v.name}`}>
                          Edit
                        </button>{' '}
                        <button className="btn btn-danger btn-sm" onClick={() => remove(v)} aria-label={`Delete ${v.name}`}>
                          <IconTrash size={12} />
                        </button>
                      </td>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
