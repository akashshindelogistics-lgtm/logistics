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
    const url: string = err.config?.url ?? '';
    // Don't redirect on auth endpoints — let the page handle the error itself
    if (err.response?.status === 401 && !url.includes('/auth/')) {
      localStorage.removeItem(TOKEN_KEY);
      localStorage.removeItem('logi_org_id');
      localStorage.removeItem('logi_org_name');
      window.location.href = '/login';
    }
    return Promise.reject(err);
  },
);

export default api;
