import api from './client';
import type { ApiResponse, Customer } from '../types';

export const listCustomers = () =>
  api.get<ApiResponse<Customer[]>>('/customers').then(r => r.data);

export const createCustomer = (name: string, address: string) =>
  api.post<ApiResponse<Customer>>('/customers', { name, address }).then(r => r.data);
