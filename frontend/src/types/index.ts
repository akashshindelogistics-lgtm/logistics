export interface Location {
  latitude: number;
  longitude: number;
  timestamp: number;
  address?: string;
}

// Mirrors Unit in src/logistics/vehicle/vehicle.rs.
export type Unit = 'MetricTon' | 'Kg' | 'Litre' | 'Box' | 'Pallet' | 'Piece';
export const UNITS: Unit[] = ['MetricTon', 'Kg', 'Litre', 'Box', 'Pallet', 'Piece'];

export interface Vehicle {
  registration_number: string;
  capacity: number;
  unit: Unit;
  location?: Location;
  assigned_driver_id?: string | null;
}

export interface Driver {
  id: string;
  org_id: string;
  name: string;
  license_number: string;
  phone: string;
  is_active: boolean;
}

export interface Stock {
  volume_in_size: number;
  quantity: number;
  description: string;
}

export interface Godown {
  id: string;
  org_id: string;
  name: string;
  address: string;
  /** Optional cap on total stored volume (Σ volume_in_size × quantity). */
  max_capacity?: number | null;
  location?: Location;
  stock: Stock[];
}

// Mirrors StockTransfer in src/logistics/godown/transfer.rs — one recorded
// move of a stock item between two godowns of the same organization.
export interface StockTransfer {
  id: string;
  org_id: string;
  from_godown_id: string;
  to_godown_id: string;
  description: string;
  quantity: number;
  volume_in_size: number;
  transferred_at: number;
}

// Mirrors ComplianceDocType / ComplianceStatus / VehicleDocument in
// src/logistics/vehicle/document.rs. `days_until_expiry` and `status` are
// computed by the server on every read.
export type ComplianceDocType =
  | 'Insurance'
  | 'RegistrationCertificate'
  | 'Permit'
  | 'PollutionCertificate'
  | 'FitnessCertificate';

export type ComplianceStatus = 'Valid' | 'ExpiringSoon' | 'Expired';

export interface VehicleDocument {
  id: string;
  org_id: string;
  vehicle_registration: string;
  doc_type: ComplianceDocType;
  document_number: string;
  issued_on: string | null;
  expires_on: string;
  notes: string | null;
  days_until_expiry: number;
  status: ComplianceStatus;
}

export interface Organization {
  id: string;
  name: string;
  address: string;
  vehicles: Vehicle[];
  godowns: Godown[];
  location?: Location;
}

export interface Customer {
  id: string;
  org_id: string;
  name: string;
  address: string;
  location?: Location;
}

// Mirrors DispatchStatus in src/logistics/dispatch/dispatch.rs. Keep in sync
// if the backend state machine changes.
export type DispatchStatus =
  | 'PENDING'
  | 'CONFIRMED'
  | 'LOADED'
  | 'IN_TRANSIT'
  | 'DELIVERED'
  | 'RETURNED'
  | 'CANCELLED';

export interface DispatchStatusEvent {
  status: DispatchStatus;
  changed_at: number;
}

export interface ProofOfDelivery {
  receiver_name: string;
  signature_or_photo_url: string;
  delivered_at: number;
}

// One stock line on a dispatch. Mirrors DispatchLineItem in
// src/logistics/dispatch/dispatch.rs.
export interface DispatchLineItem {
  stock_description: string;
  quantity: number;
  volume_in_size: number;
}

export interface DispatchOrder {
  id: string;
  org_id: string;
  customer_id: string;
  vehicle_registration_number: string;
  line_items: DispatchLineItem[];
  status: DispatchStatus;
  dispatched_at: number;
  status_history: DispatchStatusEvent[];
  proof_of_delivery: ProofOfDelivery | null;
}

export interface ApiResponse<T> {
  success: boolean;
  message: string;
  data?: T;
}
