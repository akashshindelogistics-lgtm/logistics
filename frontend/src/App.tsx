import { BrowserRouter, Routes, Route } from 'react-router-dom';
import Navbar from './components/Navbar';
import Dashboard from './pages/Dashboard';
import Organizations from './pages/Organizations';
import OrganizationDetail from './pages/OrganizationDetail';
import Vehicles from './pages/Vehicles';
import Customers from './pages/Customers';
import Dispatches from './pages/Dispatches';
import './App.css';

export default function App() {
  return (
    <BrowserRouter>
      <Navbar />
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/orgs" element={<Organizations />} />
        <Route path="/orgs/:id" element={<OrganizationDetail />} />
        <Route path="/vehicles" element={<Vehicles />} />
        <Route path="/customers" element={<Customers />} />
        <Route path="/dispatches" element={<Dispatches />} />
      </Routes>
    </BrowserRouter>
  );
}
