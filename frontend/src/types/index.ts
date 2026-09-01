export interface Location {
  latitude: number;
  longitude: number;
  timestamp: number;
  address?: string;
}

export interface Vehicle {
  registration_number: string;
  capacity: number;
  unit: 'MetricTon';
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
  location?: Location;
  stock: Stock[];
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

export interface DispatchOrder {
  id: string;
  org_id: string;
  customer_id: string;
  vehicle_registration_number: string;
  stock_description: string;
  quantity: number;
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
