import api from './client';
import type { ApiResponse, Trip } from '../types';

export interface TripStopInput {
  customer_id: string;
  line_items: { stock_description: string; requested_quantity: number }[];
}

/** Plan a multi-stop trip: one vehicle, several customer stops in sequence. */
export const createTrip = (orgId: string, stops: TripStopInput[]) =>
  api.post<ApiResponse<Trip>>(`/orgs/${orgId}/trips`, { stops }).then(r => r.data);

export const listOrgTrips = (orgId: string) =>
  api.get<ApiResponse<Trip[]>>(`/orgs/${orgId}/trips`).then(r => r.data);

export const getTrip = (tripId: string) =>
  api.get<ApiResponse<Trip>>(`/trips/${tripId}`).then(r => r.data);
