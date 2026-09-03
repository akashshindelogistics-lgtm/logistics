import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { getOrg, dispatchStock } from '../api/orgs';
import {
  addVehicle,
  deleteVehicle,
  listOrgVehicleDocuments,
  addVehicleDocument,
  updateVehicleDocument,
  deleteVehicleDocument,
} from '../api/vehicles';
import { listCustomers } from '../api/customers';
import { createGodown, deleteGodown, addGodownStock, transferGodownStock, listStockTransfers } from '../api/godowns';
import { listDrivers, addDriver, deleteDriver, updateDriver, assignVehicleDriver } from '../api/drivers';
import type {
  Organization,
  Customer,
  Driver,
  StockTransfer,
  ComplianceDocType,
  VehicleDocument,
} from '../types';
import LocationMap, { type MapPin } from '../components/LocationMap';
import { IconBuilding, IconTruck, IconPackage, IconDispatch, IconPlus, IconTrash, IconPin, IconChevron, IconCheck, IconUsers } from '../components/Icons';
import './page.css';

interface StockFormState {
  description: string;
  quantity: string;
  volumeInSize: string;
}

const emptyStockForm: StockFormState = { description: '', quantity: '', volumeInSize: '' };

interface TransferFormState {
  description: string;
  toGodownId: string;
  quantity: string;
}

const emptyTransferForm: TransferFormState = { description: '', toGodownId: '', quantity: '' };

interface DocFormState {
  vehicleRegistration: string;
  docType: ComplianceDocType;
  documentNumber: string;
  expiresOn: string;
}

const emptyDocForm: DocFormState = {
  vehicleRegistration: '',
  docType: 'Insurance',
  documentNumber: '',
  expiresOn: '',
};

const DOC_TYPE_LABELS: Record<ComplianceDocType, string> = {
  Insurance: 'Insurance',
  RegistrationCertificate: 'Registration (RC)',
  Permit: 'Permit',
  PollutionCertificate: 'Pollution (PUC)',
  FitnessCertificate: 'Fitness',
};

const COMPLIANCE_STATUS_META = {
  Valid: { label: 'Valid', className: 'tag-green' },
  ExpiringSoon: { label: 'Expiring soon', className: 'tag-amber' },
  Expired: { label: 'Expired', className: 'tag-red' },
} as const;

export default function OrganizationDetail() {
  const { id } = useParams<{ id: string }>();
  const [org, setOrg] = useState<Organization | null>(null);
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [loading, setLoading] = useState(true);

  const [vReg, setVReg] = useState('');
  const [vCap, setVCap] = useState('');
  const [vSubmitting, setVSubmitting] = useState(false);

  const [drvName, setDrvName] = useState('');
  const [drvLicense, setDrvLicense] = useState('');
  const [drvPhone, setDrvPhone] = useState('');
  const [drvSubmitting, setDrvSubmitting] = useState(false);

  const [gName, setGName] = useState('');
  const [gAddress, setGAddress] = useState('');
  const [gSubmitting, setGSubmitting] = useState(false);
  const [stockForms, setStockForms] = useState<Record<string, StockFormState>>({});
  const [stockSubmitting, setStockSubmitting] = useState<Record<string, boolean>>({});
  const [transferForms, setTransferForms] = useState<Record<string, TransferFormState>>({});
  const [transferSubmitting, setTransferSubmitting] = useState<Record<string, boolean>>({});
  const [transferMsg, setTransferMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [transfers, setTransfers] = useState<StockTransfer[]>([]);

  const [vehicleDocuments, setVehicleDocuments] = useState<VehicleDocument[]>([]);
  const [docForm, setDocForm] = useState<DocFormState>(emptyDocForm);
  const [docSubmitting, setDocSubmitting] = useState(false);
  const [docMsg, setDocMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [showDocForm, setShowDocForm] = useState(false);

  const [dCustomerId, setDCustomerId] = useState('');
  const [dLineItems, setDLineItems] = useState<{ description: string; quantity: string }[]>([
    { description: '', quantity: '' },
  ]);
  const [dMsg, setDMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [dSubmitting, setDSubmitting] = useState(false);

  const load = () =>
    Promise.all([
      getOrg(id!),
      listCustomers(),
      listDrivers(),
      listStockTransfers(id!),
      listOrgVehicleDocuments(id!),
    ])
      .then(([orgRes, custRes, drvRes, transferRes, docRes]) => {
        setOrg(orgRes.data ?? null);
        setCustomers(custRes.data ?? []);
        setDrivers(drvRes.data ?? []);
        setTransfers(transferRes.data ?? []);
        setVehicleDocuments(docRes.data ?? []);
      })
      .finally(() => setLoading(false));

  useEffect(() => { load(); }, [id]);

  const handleAddVehicle = async (e: React.FormEvent) => {
    e.preventDefault();
    setVSubmitting(true);
    try { await addVehicle(id!, vReg, Number(vCap)); setVReg(''); setVCap(''); load(); }
    finally { setVSubmitting(false); }
  };

  const handleDeleteVehicle = async (reg: string) => {
    if (!confirm(`Remove vehicle ${reg}?`)) return;
    await deleteVehicle(reg);
    load();
  };

  const handleAddDriver = async (e: React.FormEvent) => {
    e.preventDefault();
    setDrvSubmitting(true);
    try {
      await addDriver(id!, drvName, drvLicense, drvPhone);
      setDrvName(''); setDrvLicense(''); setDrvPhone('');
      load();
    } finally { setDrvSubmitting(false); }
  };

  const handleDeleteDriver = async (driverId: string, name: string) => {
    if (!confirm(`Remove driver "${name}"? Any vehicle they're assigned to will be unassigned.`)) return;
    await deleteDriver(driverId);
    load();
  };

  const handleToggleDriverActive = async (d: Driver) => {
    await updateDriver(d.id, d.name, d.license_number, d.phone, !d.is_active);
    load();
  };

  const handleAssignVehicleDriver = async (reg: string, driverId: string) => {
    await assignVehicleDriver(reg, driverId || null);
    load();
  };

  const handleAddDocument = async (e: React.FormEvent) => {
    e.preventDefault();
    setDocSubmitting(true);
    setDocMsg(null);
    try {
      await addVehicleDocument(docForm.vehicleRegistration, {
        doc_type: docForm.docType,
        document_number: docForm.documentNumber,
        expires_on: docForm.expiresOn,
      });
      setDocForm(emptyDocForm);
      setShowDocForm(false);
      setDocMsg({ text: 'Compliance document recorded.', ok: true });
      load();
    } catch (err) {
      const apiMsg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message;
      setDocMsg({ text: apiMsg ? `Could not save document: ${apiMsg}` : 'Could not save document.', ok: false });
    } finally {
      setDocSubmitting(false);
    }
  };

  const handleRenewDocument = async (doc: VehicleDocument) => {
    const next = prompt(
      `New expiry date for the ${DOC_TYPE_LABELS[doc.doc_type]} on ${doc.vehicle_registration} (YYYY-MM-DD):`,
      doc.expires_on,
    );
    if (!next || next === doc.expires_on) return;
    try {
      await updateVehicleDocument(doc.id, {
        doc_type: doc.doc_type,
        document_number: doc.document_number,
        issued_on: doc.issued_on,
        expires_on: next,
        notes: doc.notes,
      });
      setDocMsg({ text: 'Document renewed.', ok: true });
      load();
    } catch (err) {
      const apiMsg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message;
      setDocMsg({ text: apiMsg ? `Renewal failed: ${apiMsg}` : 'Renewal failed.', ok: false });
    }
  };

  const handleDeleteDocument = async (doc: VehicleDocument) => {
    if (!confirm(`Delete the ${DOC_TYPE_LABELS[doc.doc_type]} record for ${doc.vehicle_registration}?`)) return;
    await deleteVehicleDocument(doc.id);
    load();
  };

  const handleAddGodown = async (e: React.FormEvent) => {
    e.preventDefault();
    setGSubmitting(true);
    try { await createGodown(id!, gName, gAddress); setGName(''); setGAddress(''); load(); }
    finally { setGSubmitting(false); }
  };

  const handleDeleteGodown = async (godownId: string, name: string) => {
    if (!confirm(`Remove godown "${name}" and all its stock?`)) return;
    await deleteGodown(godownId);
    load();
  };

  const stockFormFor = (godownId: string) => stockForms[godownId] ?? emptyStockForm;

  const updateStockForm = (godownId: string, field: keyof StockFormState, value: string) => {
    setStockForms(prev => ({ ...prev, [godownId]: { ...stockFormFor(godownId), [field]: value } }));
  };

  const handleAddStock = async (e: React.FormEvent, godownId: string) => {
    e.preventDefault();
    const form = stockFormFor(godownId);
    setStockSubmitting(prev => ({ ...prev, [godownId]: true }));
    try {
      await addGodownStock(godownId, form.description, Number(form.quantity), Number(form.volumeInSize));
      setStockForms(prev => ({ ...prev, [godownId]: emptyStockForm }));
      load();
    } finally {
      setStockSubmitting(prev => ({ ...prev, [godownId]: false }));
    }
  };

  const transferFormFor = (godownId: string) => transferForms[godownId] ?? emptyTransferForm;

  const updateTransferForm = (godownId: string, field: keyof TransferFormState, value: string) => {
    setTransferForms(prev => ({ ...prev, [godownId]: { ...transferFormFor(godownId), [field]: value } }));
  };

  const handleTransfer = async (e: React.FormEvent, fromGodownId: string) => {
    e.preventDefault();
    const form = transferFormFor(fromGodownId);
    setTransferSubmitting(prev => ({ ...prev, [fromGodownId]: true }));
    setTransferMsg(null);
    try {
      await transferGodownStock(fromGodownId, form.toGodownId, form.description, Number(form.quantity));
      setTransferForms(prev => ({ ...prev, [fromGodownId]: emptyTransferForm }));
      setTransferMsg({ text: 'Stock transferred between godowns.', ok: true });
      load();
    } catch (err) {
      const apiMsg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message;
      setTransferMsg({ text: apiMsg ? `Transfer failed: ${apiMsg}` : 'Transfer failed.', ok: false });
    } finally {
      setTransferSubmitting(prev => ({ ...prev, [fromGodownId]: false }));
    }
  };

  const updateLineItem = (index: number, field: 'description' | 'quantity', value: string) => {
    setDLineItems(prev => prev.map((li, i) => (i === index ? { ...li, [field]: value } : li)));
  };
  const addLineItem = () => setDLineItems(prev => [...prev, { description: '', quantity: '' }]);
  const removeLineItem = (index: number) =>
    setDLineItems(prev => (prev.length > 1 ? prev.filter((_, i) => i !== index) : prev));

  const handleDispatch = async (e: React.FormEvent) => {
    e.preventDefault();
    setDSubmitting(true); setDMsg(null);
    try {
      const lineItems = dLineItems
        .filter(li => li.description.trim() && li.quantity)
        .map(li => ({ stockDescription: li.description.trim(), requestedQuantity: Number(li.quantity) }));
      if (lineItems.length === 0) {
        setDMsg({ text: 'Add at least one stock line with a description and quantity.', ok: false });
        return;
      }
      await dispatchStock(id!, dCustomerId, lineItems);
      setDMsg({ text: 'Dispatch successful! Stock is on its way.', ok: true });
      setDLineItems([{ description: '', quantity: '' }]);
      load();
    } catch (err) {
      const apiMsg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message;
      setDMsg({
        text: apiMsg
          ? `Dispatch failed: ${apiMsg}`
          : 'Dispatch failed. Check stock quantity, customer location, and that a vehicle with an active driver and enough capacity is free.',
        ok: false,
      });
    } finally { setDSubmitting(false); }
  };

  if (loading) {
    return (
      <div className="page">
        <div className="skeleton" style={{ width: 200, height: 28, marginBottom: 8 }} />
        <div className="skeleton" style={{ width: 300, height: 16, marginBottom: 28 }} />
        <div className="skeleton" style={{ width: '100%', height: 300, borderRadius: 12 }} />
      </div>
    );
  }

  if (!org) {
    return (
      <div className="page">
        <div className="empty-state">
          <div className="empty-state-icon"><IconBuilding size={28} /></div>
          <h3>Organization not found</h3>
          <p>This organization may have been deleted.</p>
          <Link to="/orgs" className="btn btn-primary">Back to Organizations</Link>
        </div>
      </div>
    );
  }

  const godownName = (godownId: string) =>
    org.godowns.find(g => g.id === godownId)?.name ?? 'a removed godown';

  const expiringSoonCount = vehicleDocuments.filter(d => d.status === 'ExpiringSoon').length;
  const expiredCount = vehicleDocuments.filter(d => d.status === 'Expired').length;

  const mapPins: MapPin[] = [];
  if (org.location)
    mapPins.push({ lat: org.location.latitude, lng: org.location.longitude, label: org.name, detail: org.address });
  org.vehicles.forEach(v => {
    if (v.location)
      mapPins.push({ lat: v.location.latitude, lng: v.location.longitude, label: v.registration_number, detail: `${v.capacity} ${v.unit}` });
  });
  org.godowns.forEach(g => {
    if (g.location)
      mapPins.push({ lat: g.location.latitude, lng: g.location.longitude, label: g.name, detail: g.address });
  });

  return (
    <div className="page">
      {/* Breadcrumb */}
      <nav style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13, color: 'var(--text-3)', marginBottom: 18 }}>
        <Link to="/orgs" style={{ color: 'var(--text-3)', textDecoration: 'none' }}>Organizations</Link>
        <IconChevron size={12} />
        <span style={{ color: 'var(--text-1)' }}>{org.name}</span>
      </nav>

      {/* Page header */}
      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ width: 48, height: 48, borderRadius: 12, background: 'var(--blue-bg)', color: 'var(--blue)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
            <IconBuilding size={22} />
          </div>
          <div>
            <h1 style={{ margin: 0 }}>{org.name}</h1>
            <p style={{ margin: 0, marginTop: 2, display: 'flex', alignItems: 'center', gap: 5 }}>
              <IconPin size={12} />
              {org.address}
            </p>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 10 }}>
          <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '8px 14px', fontSize: 13, textAlign: 'center' }}>
            <div style={{ fontWeight: 700, fontSize: 18, color: 'var(--blue)' }}>{org.vehicles.length}</div>
            <div className="muted">Vehicles</div>
          </div>
          <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '8px 14px', fontSize: 13, textAlign: 'center' }}>
            <div style={{ fontWeight: 700, fontSize: 18, color: 'var(--purple)' }}>{org.godowns.length}</div>
            <div className="muted">Godowns</div>
          </div>
        </div>
      </div>

      {/* Map */}
      {mapPins.length > 0 && (
        <div className="section-card" style={{ marginBottom: 24 }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconPin size={15} />Location Map</span>
            <span className="badge tag-blue">{mapPins.length} pins</span>
          </div>
          <LocationMap pins={mapPins} height="320px" />
        </div>
      )}

      <div className="detail-grid">
        {/* Vehicles */}
        <div className="section-card" style={{ gridColumn: '1 / -1' }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconTruck size={15} />Fleet Vehicles</span>
            <span className="badge">{org.vehicles.length}</span>
          </div>

          {org.vehicles.length === 0 ? (
            <div className="empty-state" style={{ padding: '28px 20px' }}>
              <div className="empty-state-icon"><IconTruck size={22} /></div>
              <h3>No vehicles</h3>
              <p>Add vehicles to this organization below.</p>
            </div>
          ) : (
            <div className="table-wrap">
              <table data-testid="fleet-table">
                <thead>
                  <tr><th>Registration</th><th>Capacity</th><th>Driver</th><th>Coordinates</th><th>Last Seen</th><th></th></tr>
                </thead>
                <tbody>
                  {org.vehicles.map(v => (
                    <tr key={v.registration_number}>
                      <td>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                          <div style={{ width: 28, height: 28, borderRadius: 6, background: 'var(--green-bg)', color: 'var(--green)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                            <IconTruck size={13} />
                          </div>
                          <span className="entity-name">{v.registration_number}</span>
                        </div>
                      </td>
                      <td><span className="badge tag-blue">{v.capacity} {v.unit}</span></td>
                      <td>
                        <select
                          aria-label={`Driver for ${v.registration_number}`}
                          value={v.assigned_driver_id ?? ''}
                          onChange={e => handleAssignVehicleDriver(v.registration_number, e.target.value)}
                        >
                          <option value="">Unassigned</option>
                          {drivers.map(d => (
                            <option key={d.id} value={d.id}>
                              {d.name}{d.is_active ? '' : ' (inactive)'}
                            </option>
                          ))}
                        </select>
                      </td>
                      <td className="coord-cell">
                        {v.location
                          ? <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}><IconPin size={11} />{v.location.latitude.toFixed(4)}, {v.location.longitude.toFixed(4)}</span>
                          : <span className="muted">Not tracked</span>}
                      </td>
                      <td className="muted">{v.location ? new Date(v.location.timestamp * 1000).toLocaleString() : '—'}</td>
                      <td>
                        <button className="btn btn-danger btn-sm" onClick={() => handleDeleteVehicle(v.registration_number)}>
                          <IconTrash size={12} />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* Add vehicle inline */}
          <div style={{ padding: '16px 20px', borderTop: '1px solid var(--border)', background: 'var(--bg)' }}>
            <p style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 12 }}>Add Vehicle</p>
            <form onSubmit={handleAddVehicle} style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
              <div className="field" style={{ flex: 1, minWidth: 160, marginBottom: 0 }}>
                <label htmlFor="v-reg">Registration Number</label>
                <input id="v-reg" placeholder="e.g. MH12AB1234" value={vReg} onChange={e => setVReg(e.target.value)} required />
              </div>
              <div className="field" style={{ flex: '0 0 140px', marginBottom: 0 }}>
                <label htmlFor="v-cap">Capacity (MT)</label>
                <input id="v-cap" type="number" placeholder="e.g. 10" value={vCap} onChange={e => setVCap(e.target.value)} required />
              </div>
              <button className="btn btn-primary" type="submit" disabled={vSubmitting} style={{ alignSelf: 'flex-end' }}>
                <IconPlus size={14} />{vSubmitting ? 'Adding…' : 'Add Vehicle'}
              </button>
            </form>
          </div>
        </div>

        {/* Drivers */}
        <div className="section-card" style={{ gridColumn: '1 / -1' }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconUsers size={15} />Drivers</span>
            <span className="badge">{drivers.length}</span>
          </div>

          {drivers.length === 0 ? (
            <div className="empty-state" style={{ padding: '28px 20px' }}>
              <div className="empty-state-icon"><IconUsers size={22} /></div>
              <h3>No drivers</h3>
              <p>Add a driver below, then assign one to a vehicle. A vehicle needs an active driver before it can be dispatched.</p>
            </div>
          ) : (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr><th>Name</th><th>Licence</th><th>Phone</th><th>Status</th><th></th></tr>
                </thead>
                <tbody>
                  {drivers.map(d => (
                    <tr key={d.id}>
                      <td>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                          <div style={{ width: 28, height: 28, borderRadius: 6, background: 'var(--blue-bg)', color: 'var(--blue)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                            <IconUsers size={13} />
                          </div>
                          <Link to={`/drivers/${d.id}`} className="entity-name" style={{ textDecoration: 'none', color: 'var(--blue)' }}>{d.name}</Link>
                        </div>
                      </td>
                      <td className="muted">{d.license_number}</td>
                      <td className="muted">{d.phone}</td>
                      <td>
                        <button
                          className={`badge ${d.is_active ? 'tag-green' : ''}`}
                          onClick={() => handleToggleDriverActive(d)}
                          title="Toggle active status"
                          style={{ cursor: 'pointer', border: 'none' }}
                        >
                          {d.is_active ? 'Active' : 'Inactive'}
                        </button>
                      </td>
                      <td>
                        <button className="btn btn-danger btn-sm" onClick={() => handleDeleteDriver(d.id, d.name)}>
                          <IconTrash size={12} />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <div style={{ padding: '16px 20px', borderTop: '1px solid var(--border)', background: 'var(--bg)' }}>
            <p style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 12 }}>Add Driver</p>
            <form onSubmit={handleAddDriver} style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
              <div className="field" style={{ flex: 1, minWidth: 140, marginBottom: 0 }}>
                <label htmlFor="drv-name">Driver Name</label>
                <input id="drv-name" placeholder="e.g. Ravi Kumar" value={drvName} onChange={e => setDrvName(e.target.value)} required />
              </div>
              <div className="field" style={{ flex: 1, minWidth: 140, marginBottom: 0 }}>
                <label htmlFor="drv-license">Licence Number</label>
                <input id="drv-license" placeholder="e.g. DL-1420110012345" value={drvLicense} onChange={e => setDrvLicense(e.target.value)} required />
              </div>
              <div className="field" style={{ flex: '0 0 160px', marginBottom: 0 }}>
                <label htmlFor="drv-phone">Phone</label>
                <input id="drv-phone" placeholder="e.g. +91 98100 00000" value={drvPhone} onChange={e => setDrvPhone(e.target.value)} required />
              </div>
              <button className="btn btn-primary" type="submit" disabled={drvSubmitting} style={{ alignSelf: 'flex-end' }}>
                <IconPlus size={14} />{drvSubmitting ? 'Adding…' : 'Add Driver'}
              </button>
            </form>
          </div>
        </div>

        {/* Vehicle compliance paperwork */}
        <div className="section-card" style={{ gridColumn: '1 / -1' }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconTruck size={15} />Vehicle Compliance</span>
            <span style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
              <span className="badge">{vehicleDocuments.length}</span>
              {expiringSoonCount > 0 && (
                <span className="badge tag-amber">{expiringSoonCount} expiring soon</span>
              )}
              {expiredCount > 0 && (
                <span className="badge tag-red">{expiredCount} expired</span>
              )}
            </span>
          </div>

          {docMsg && (
            <div
              role="status"
              style={{ margin: '12px 20px 0', padding: '10px 14px', borderRadius: 8, fontSize: 13, background: docMsg.ok ? 'var(--green-bg)' : 'var(--red-bg)', color: docMsg.ok ? 'var(--green-text)' : 'var(--red-text)', display: 'flex', alignItems: 'center', gap: 8 }}
            >
              {docMsg.ok && <IconCheck size={14} />}
              {docMsg.text}
            </div>
          )}

          {org.vehicles.length > 0 && (
            <div style={{ padding: '12px 20px 0' }}>
              <button
                type="button"
                className="btn btn-sm"
                aria-expanded={showDocForm}
                onClick={() => { setShowDocForm(v => !v); setDocMsg(null); }}
              >
                <IconPlus size={13} />{showDocForm ? 'Close' : 'Add compliance document'}
              </button>
            </div>
          )}

          {org.vehicles.length === 0 ? (
            <div className="empty-state" style={{ padding: '28px 20px' }}>
              <div className="empty-state-icon"><IconTruck size={22} /></div>
              <h3>Add a vehicle to track compliance</h3>
              <p>Record each truck's insurance, RC, permit, PUC and fitness paperwork once it's in the fleet.</p>
            </div>
          ) : vehicleDocuments.length === 0 ? (
            <div className="empty-state" style={{ padding: '28px 20px' }}>
              <div className="empty-state-icon"><IconTruck size={22} /></div>
              <h3>No compliance documents recorded</h3>
              <p>Track each vehicle's papers and their expiry dates below.</p>
            </div>
          ) : (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr><th>Vehicle</th><th>Document</th><th>Number</th><th>Expires</th><th>Status</th><th></th></tr>
                </thead>
                <tbody>
                  {vehicleDocuments.map(doc => {
                    const meta = COMPLIANCE_STATUS_META[doc.status];
                    return (
                      <tr key={doc.id} data-testid="compliance-row">
                        <td><span className="entity-name">{doc.vehicle_registration}</span></td>
                        <td>{DOC_TYPE_LABELS[doc.doc_type]}</td>
                        <td className="muted">{doc.document_number}</td>
                        <td>
                          {doc.expires_on}
                          <span className="muted" style={{ marginLeft: 6, fontSize: 12 }}>
                            {doc.days_until_expiry < 0
                              ? `(${-doc.days_until_expiry}d ago)`
                              : `(in ${doc.days_until_expiry}d)`}
                          </span>
                        </td>
                        <td><span className={`badge ${meta.className}`}>{meta.label}</span></td>
                        <td>
                          <div style={{ display: 'flex', gap: 6 }}>
                            <button className="btn btn-sm" onClick={() => handleRenewDocument(doc)}>Renew</button>
                            <button className="btn btn-danger btn-sm" onClick={() => handleDeleteDocument(doc)}>
                              <IconTrash size={12} />
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}

          {org.vehicles.length > 0 && showDocForm && (
            <div style={{ padding: '16px 20px', borderTop: '1px solid var(--border)', background: 'var(--bg)' }}>
              <p style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 12 }}>Add Compliance Document</p>
              <form onSubmit={handleAddDocument} style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
                <div className="field" style={{ flex: '0 0 170px', marginBottom: 0 }}>
                  <label htmlFor="doc-vehicle">Vehicle</label>
                  <select
                    id="doc-vehicle"
                    value={docForm.vehicleRegistration}
                    onChange={e => setDocForm(f => ({ ...f, vehicleRegistration: e.target.value }))}
                    required
                  >
                    <option value="">Select…</option>
                    {org.vehicles.map(v => (
                      <option key={v.registration_number} value={v.registration_number}>{v.registration_number}</option>
                    ))}
                  </select>
                </div>
                <div className="field" style={{ flex: '0 0 160px', marginBottom: 0 }}>
                  <label htmlFor="doc-type">Document</label>
                  <select
                    id="doc-type"
                    value={docForm.docType}
                    onChange={e => setDocForm(f => ({ ...f, docType: e.target.value as ComplianceDocType }))}
                  >
                    {(Object.keys(DOC_TYPE_LABELS) as ComplianceDocType[]).map(t => (
                      <option key={t} value={t}>{DOC_TYPE_LABELS[t]}</option>
                    ))}
                  </select>
                </div>
                <div className="field" style={{ flex: 1, minWidth: 140, marginBottom: 0 }}>
                  <label htmlFor="doc-number">Document Number</label>
                  <input id="doc-number" value={docForm.documentNumber} onChange={e => setDocForm(f => ({ ...f, documentNumber: e.target.value }))} required />
                </div>
                <div className="field" style={{ flex: '0 0 160px', marginBottom: 0 }}>
                  <label htmlFor="doc-expiry">Expiry Date</label>
                  <input id="doc-expiry" type="date" value={docForm.expiresOn} onChange={e => setDocForm(f => ({ ...f, expiresOn: e.target.value }))} required />
                </div>
                <button className="btn btn-primary" type="submit" disabled={docSubmitting} style={{ alignSelf: 'flex-end' }}>
                  <IconPlus size={14} />{docSubmitting ? 'Saving…' : 'Save Document'}
                </button>
              </form>
            </div>
          )}
        </div>

        {/* Godowns (warehouses) and their stock */}
        <div className="section-card" style={{ gridColumn: '1 / -1' }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconBuilding size={15} />Godowns</span>
            <span className="badge">{org.godowns.length}</span>
          </div>

          {transferMsg && (
            <div style={{ margin: '12px 20px 0', padding: '10px 14px', borderRadius: 8, fontSize: 13, background: transferMsg.ok ? 'var(--green-bg)' : 'var(--red-bg)', color: transferMsg.ok ? 'var(--green)' : 'var(--red)', display: 'flex', alignItems: 'center', gap: 8 }}>
              {transferMsg.ok && <IconCheck size={14} />}
              {transferMsg.text}
            </div>
          )}

          {org.godowns.length === 0 ? (
            <div className="empty-state" style={{ padding: '28px 20px' }}>
              <div className="empty-state-icon"><IconBuilding size={22} /></div>
              <h3>No godowns</h3>
              <p>Add a godown below, then add stock to it.</p>
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16, padding: '16px 20px' }}>
              {org.godowns.map(g => {
                const form = stockFormFor(g.id);
                const submitting = !!stockSubmitting[g.id];
                return (
                  <div key={g.id} data-testid="godown-card" style={{ border: '1px solid var(--border)', borderRadius: 10, overflow: 'hidden' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '10px 14px', borderBottom: '1px solid var(--border)', background: 'var(--bg)' }}>
                      <div>
                        <Link to={`/godowns/${g.id}`} style={{ fontWeight: 700, textDecoration: 'none', color: 'var(--blue)' }}>{g.name}</Link>
                        <div className="muted" style={{ fontSize: 12, display: 'flex', alignItems: 'center', gap: 4, marginTop: 2 }}>
                          <IconPin size={11} />{g.address}
                        </div>
                      </div>
                      <button className="btn btn-danger btn-sm" onClick={() => handleDeleteGodown(g.id, g.name)}>
                        <IconTrash size={12} />
                      </button>
                    </div>

                    {g.stock.length === 0 ? (
                      <p className="muted" style={{ padding: '14px', margin: 0, fontSize: 13 }}>No stock in this godown yet.</p>
                    ) : (
                      <div className="table-wrap">
                        <table>
                          <thead>
                            <tr><th>Description</th><th>Quantity</th><th>Volume</th></tr>
                          </thead>
                          <tbody>
                            {g.stock.map(s => (
                              <tr key={s.description}>
                                <td>
                                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                                    <div style={{ width: 26, height: 26, borderRadius: 6, background: 'var(--purple-bg)', color: 'var(--purple)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                                      <IconPackage size={12} />
                                    </div>
                                    <span className="entity-name">{s.description}</span>
                                  </div>
                                </td>
                                <td><span style={{ fontWeight: 700, color: 'var(--text-1)' }}>{s.quantity}</span> <span className="muted">units</span></td>
                                <td><span className="badge tag-blue">{s.volume_in_size}</span></td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}

                    <form
                      onSubmit={e => handleAddStock(e, g.id)}
                      style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap', padding: '12px 14px', borderTop: '1px solid var(--border)' }}
                    >
                      <div className="field" style={{ flex: 1, minWidth: 140, marginBottom: 0 }}>
                        <label htmlFor={`stock-desc-${g.id}`}>Stock Item</label>
                        <input
                          id={`stock-desc-${g.id}`}
                          placeholder="What is being stored?"
                          value={form.description}
                          onChange={e => updateStockForm(g.id, 'description', e.target.value)}
                          required
                        />
                      </div>
                      <div className="field" style={{ flex: '0 0 100px', marginBottom: 0 }}>
                        <label htmlFor={`stock-qty-${g.id}`}>Stock Quantity</label>
                        <input
                          id={`stock-qty-${g.id}`}
                          type="number"
                          value={form.quantity}
                          onChange={e => updateStockForm(g.id, 'quantity', e.target.value)}
                          required
                        />
                      </div>
                      <div className="field" style={{ flex: '0 0 100px', marginBottom: 0 }}>
                        <label htmlFor={`stock-vol-${g.id}`}>Volume</label>
                        <input
                          id={`stock-vol-${g.id}`}
                          type="number"
                          value={form.volumeInSize}
                          onChange={e => updateStockForm(g.id, 'volumeInSize', e.target.value)}
                          required
                        />
                      </div>
                      <button className="btn btn-primary btn-sm" type="submit" disabled={submitting}>
                        <IconPlus size={12} />{submitting ? 'Adding…' : 'Add Stock'}
                      </button>
                    </form>

                    {org.godowns.length > 1 && g.stock.length > 0 && (
                      <form
                        onSubmit={e => handleTransfer(e, g.id)}
                        style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap', padding: '12px 14px', borderTop: '1px dashed var(--border)', background: 'var(--bg)' }}
                      >
                        <div className="field" style={{ flex: 1, minWidth: 140, marginBottom: 0 }}>
                          <label htmlFor={`transfer-item-${g.id}`}>Transfer Item</label>
                          <select
                            id={`transfer-item-${g.id}`}
                            value={transferFormFor(g.id).description}
                            onChange={e => updateTransferForm(g.id, 'description', e.target.value)}
                            required
                          >
                            <option value="">Select an item…</option>
                            {g.stock.map(s => (
                              <option key={s.description} value={s.description}>{s.description}</option>
                            ))}
                          </select>
                        </div>
                        <div className="field" style={{ flex: 1, minWidth: 140, marginBottom: 0 }}>
                          <label htmlFor={`transfer-dest-${g.id}`}>To Godown</label>
                          <select
                            id={`transfer-dest-${g.id}`}
                            value={transferFormFor(g.id).toGodownId}
                            onChange={e => updateTransferForm(g.id, 'toGodownId', e.target.value)}
                            required
                          >
                            <option value="">Select a godown…</option>
                            {org.godowns.filter(other => other.id !== g.id).map(other => (
                              <option key={other.id} value={other.id}>{other.name}</option>
                            ))}
                          </select>
                        </div>
                        <div className="field" style={{ flex: '0 0 110px', marginBottom: 0 }}>
                          <label htmlFor={`transfer-qty-${g.id}`}>Transfer Quantity</label>
                          <input
                            id={`transfer-qty-${g.id}`}
                            type="number"
                            value={transferFormFor(g.id).quantity}
                            onChange={e => updateTransferForm(g.id, 'quantity', e.target.value)}
                            required
                          />
                        </div>
                        <button className="btn btn-sm" type="submit" disabled={!!transferSubmitting[g.id]}>
                          <IconDispatch size={12} />{transferSubmitting[g.id] ? 'Moving…' : 'Transfer'}
                        </button>
                      </form>
                    )}
                  </div>
                );
              })}
            </div>
          )}

          {/* Add godown inline */}
          <div style={{ padding: '16px 20px', borderTop: '1px solid var(--border)', background: 'var(--bg)' }}>
            <p style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 12 }}>Add Godown</p>
            <form onSubmit={handleAddGodown} style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
              <div className="field" style={{ flex: 1, minWidth: 160, marginBottom: 0 }}>
                <label htmlFor="g-name">Godown Name</label>
                <input id="g-name" placeholder="e.g. North Godown" value={gName} onChange={e => setGName(e.target.value)} required />
              </div>
              <div className="field" style={{ flex: 1, minWidth: 160, marginBottom: 0 }}>
                <label htmlFor="g-address">Address</label>
                <input id="g-address" placeholder="e.g. Plot 5, Industrial Area" value={gAddress} onChange={e => setGAddress(e.target.value)} required />
              </div>
              <button className="btn btn-primary" type="submit" disabled={gSubmitting} style={{ alignSelf: 'flex-end' }}>
                <IconPlus size={14} />{gSubmitting ? 'Adding…' : 'Add Godown'}
              </button>
            </form>
          </div>
        </div>

        {/* Godown-to-godown transfer history */}
        {transfers.length > 0 && (
          <div className="section-card" style={{ gridColumn: '1 / -1' }}>
            <div className="section-card-header">
              <span className="section-card-title"><IconDispatch size={15} />Stock Transfers</span>
              <span className="badge">{transfers.length}</span>
            </div>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr><th>Item</th><th>From</th><th>To</th><th>Quantity</th><th>When</th></tr>
                </thead>
                <tbody>
                  {transfers.map(t => (
                    <tr key={t.id}>
                      <td><span className="entity-name">{t.description}</span></td>
                      <td className="muted">{godownName(t.from_godown_id)}</td>
                      <td className="muted">{godownName(t.to_godown_id)}</td>
                      <td><span style={{ fontWeight: 700 }}>{t.quantity}</span> <span className="muted">units</span></td>
                      <td className="muted">{new Date(t.transferred_at * 1000).toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {/* Dispatch form */}
        <div className="section-card">
          <div className="section-card-header">
            <span className="section-card-title"><IconDispatch size={15} />Dispatch Stock</span>
          </div>
          <div style={{ padding: '20px' }}>
            <form onSubmit={handleDispatch}>
              <div className="field">
                <label htmlFor="d-customer">Customer</label>
                <select id="d-customer" value={dCustomerId} onChange={e => setDCustomerId(e.target.value)} required>
                  <option value="">Select a customer…</option>
                  {customers.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
                </select>
              </div>
              <p style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: '0.08em', margin: '4px 0 10px' }}>Stock Lines</p>
              {dLineItems.map((li, i) => {
                const suffix = i === 0 ? '' : ` ${i + 1}`;
                return (
                  <div key={i} style={{ display: 'flex', gap: 8, alignItems: 'flex-end', marginBottom: 10 }} data-testid="dispatch-line-item">
                    <div className="field" style={{ flex: 1, marginBottom: 0 }}>
                      <label htmlFor={`d-stock-${i}`}>{`Stock Description${suffix}`}</label>
                      <input
                        id={`d-stock-${i}`}
                        placeholder="What is being dispatched?"
                        value={li.description}
                        onChange={e => updateLineItem(i, 'description', e.target.value)}
                      />
                    </div>
                    <div className="field" style={{ flex: '0 0 130px', marginBottom: 0 }}>
                      <label htmlFor={`d-qty-${i}`}>{`Quantity${suffix}`}</label>
                      <input
                        id={`d-qty-${i}`}
                        type="number"
                        placeholder="Units"
                        value={li.quantity}
                        onChange={e => updateLineItem(i, 'quantity', e.target.value)}
                      />
                    </div>
                    {dLineItems.length > 1 && (
                      <button
                        type="button"
                        className="btn btn-danger btn-sm"
                        aria-label={`Remove stock line ${i + 1}`}
                        onClick={() => removeLineItem(i)}
                        style={{ marginBottom: 2 }}
                      >
                        <IconTrash size={12} />
                      </button>
                    )}
                  </div>
                );
              })}
              <button type="button" className="btn btn-sm" onClick={addLineItem} style={{ marginBottom: 14 }}>
                <IconPlus size={13} />Add another line
              </button>

              {dMsg && (
                <div style={{ padding: '10px 14px', borderRadius: 8, marginBottom: 14, fontSize: 13, background: dMsg.ok ? 'var(--green-bg)' : 'var(--red-bg)', color: dMsg.ok ? 'var(--green)' : 'var(--red)', display: 'flex', alignItems: 'center', gap: 8 }}>
                  {dMsg.ok && <IconCheck size={14} />}
                  {dMsg.text}
                </div>
              )}

              <button className="btn btn-primary" type="submit" disabled={dSubmitting} style={{ width: '100%', justifyContent: 'center' }}>
                <IconDispatch size={14} />{dSubmitting ? 'Dispatching…' : 'Dispatch Stock'}
              </button>
            </form>
          </div>
        </div>
      </div>
    </div>
  );
}
