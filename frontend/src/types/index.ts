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
}

export interface Stock {
  volume_in_size: number;
  quantity: number;
  description: string;
}

export interface Organization {
  id: string;
  name: string;
  address: string;
  vehicles: Vehicle[];
  stock: Stock[];
  location?: Location;
}

export interface Customer {
  id: string;
  name: string;
  address: string;
  location?: Location;
}

export interface DispatchOrder {
  id: string;
  org_id: string;
  customer_id: string;
  vehicle_registration_number: string;
  stock_description: string;
  quantity: number;
  status: string;
  dispatched_at: number;
}

export interface ApiResponse<T> {
  success: boolean;
  message: string;
  data?: T;
}
