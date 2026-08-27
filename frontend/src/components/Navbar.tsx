import { NavLink } from 'react-router-dom';
import './Navbar.css';

const links = [
  { to: '/', label: 'Dashboard' },
  { to: '/orgs', label: 'Organizations' },
  { to: '/vehicles', label: 'Vehicles' },
  { to: '/customers', label: 'Customers' },
  { to: '/dispatches', label: 'Dispatches' },
];

export default function Navbar() {
  return (
    <nav className="navbar">
      <span className="navbar-brand">Logistics System</span>
      <ul className="navbar-links">
        {links.map(l => (
          <li key={l.to}>
            <NavLink to={l.to} end={l.to === '/'} className={({ isActive }) => isActive ? 'active' : ''}>
              {l.label}
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  );
}
