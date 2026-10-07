import api from './client';
import type { ApiResponse, Trip } from '../types';

export interface TripStopInput {
  customer_id: string;
  line_items: { stock_description: string; requested_quantity: number }[];
}

/**
 * Plan a multi-stop trip: one vehicle, several customer stops in sequence.
 * When `optimizeRoute` is true, the server reorders stops 2..N by
 * nearest-neighbour geography (the first stop always stays fixed as the
 * route's starting point).
 */
export const createTrip = (
  orgId: string,
  stops: TripStopInput[],
  optimizeRoute = false,
  hireVendorId?: string,
) =>
  api
    .post<ApiResponse<Trip>>(`/orgs/${orgId}/trips`, {
      stops,
      optimize_route: optimizeRoute,
      ...(hireVendorId ? { vehicle_source: 'HIRED', vendor_id: hireVendorId } : {}),
    })
    .then(r => r.data);

export const listOrgTrips = (orgId: string) =>
  api.get<ApiResponse<Trip[]>>(`/orgs/${orgId}/trips`).then(r => r.data);

export const getTrip = (tripId: string) =>
  api.get<ApiResponse<Trip>>(`/trips/${tripId}`).then(r => r.data);
