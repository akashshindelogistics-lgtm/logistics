import axios from 'axios';

const TOKEN_KEY = 'logi_token';

// In dev the Vite proxy forwards `/api` to the local backend. In a deployed
// build (e.g. the static site on GitHub Pages) the API lives on another origin,
// so point VITE_API_BASE_URL at it — e.g. https://logi-api.duckdns.org/api
const api = axios.create({
  baseURL: import.meta.env.VITE_API_BASE_URL || '/api',
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
