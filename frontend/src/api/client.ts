import axios from 'axios';

const TOKEN_KEY = 'logi_token';

const api = axios.create({
  baseURL: '/api',
  headers: { 'Content-Type': 'application/json' },
});

api.interceptors.request.use(config => {
  const token = localStorage.getItem(TOKEN_KEY);
  if (token) config.headers.Authorization = `Bearer ${token}`;
  return config;
});

api.interceptors.response.use(
  r => r,
  err => {
    if (err.response?.status === 401) {
      localStorage.removeItem(TOKEN_KEY);
      localStorage.removeItem('logi_org_id');
      localStorage.removeItem('logi_org_name');
      window.location.href = '/login';
    }
    return Promise.reject(err);
  },
);

export default api;
