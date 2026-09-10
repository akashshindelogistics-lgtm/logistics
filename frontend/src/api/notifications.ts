import api from './client';
import type { ApiResponse, Notification } from '../types';

/** Notifications recorded for one dispatch (customer + driver, created + delivered). */
export const listDispatchNotifications = (dispatchId: string) =>
  api.get<ApiResponse<Notification[]>>(`/dispatches/${dispatchId}/notifications`).then(r => r.data);

/** The org's 100 most recent notifications, newest first. */
export const listOrgNotifications = (orgId: string) =>
  api.get<ApiResponse<Notification[]>>(`/orgs/${orgId}/notifications`).then(r => r.data);
