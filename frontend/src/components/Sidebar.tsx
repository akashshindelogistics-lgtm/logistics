import { NavLink, useNavigate } from 'react-router-dom';
import { IconGrid, IconBuilding, IconTruck, IconUsers, IconPackage, IconDispatch, IconX } from './Icons';
import { getOrgName, getOrgId, clearAuth, isLoggedIn } from '../api/auth';
import './Sidebar.css';

const links = [
  { to: '/',           label: 'Dashboard',      Icon: IconGrid },
  { to: '/orgs',       label: 'My Organization', Icon: IconBuilding },
  { to: '/vehicles',   label: 'Vehicles',        Icon: IconTruck },
  { to: '/customers',  label: 'Customers',       Icon: IconUsers },
  { to: '/dispatches', label: 'Dispatches',      Icon: IconDispatch },
];

export default function Sidebar() {
  const navigate = useNavigate();
  const loggedIn = isLoggedIn();
  const orgName = getOrgName();
  const orgId = getOrgId();

  const handleLogout = () => {
    clearAuth();
    navigate('/login', { replace: true });
  };

  return (
    <aside className="sidebar">
      <div className="sidebar-logo">
        <div className="sidebar-logo-icon">
          <IconPackage size={20} />
        </div>
        <div className="sidebar-logo-text">
          <span className="sidebar-logo-name">LogiTrack</span>
          <span className="sidebar-logo-sub">Logistics System</span>
        </div>
      </div>

      {loggedIn && orgName && (
        <div className="sidebar-org-badge">
          <div className="sidebar-org-avatar">{orgName.charAt(0).toUpperCase()}</div>
          <div className="sidebar-org-info">
            <span className="sidebar-org-name">{orgName}</span>
            <span className="sidebar-org-id">{orgId?.slice(0, 8)}…</span>
          </div>
        </div>
      )}

      <nav className="sidebar-nav">
        <p className="sidebar-section-label">Navigation</p>
        {links.map(({ to, label, Icon }) => {
          // Redirect /orgs to the specific org detail page if we know the org ID
          const href = (to === '/orgs' && orgId) ? `/orgs/${orgId}` : to;
          return (
            <NavLink
              key={to}
              to={href}
              end={to === '/'}
              className={({ isActive }) => `sidebar-link${isActive ? ' active' : ''}`}
            >
              <Icon size={16} />
              <span>{label}</span>
            </NavLink>
          );
        })}
      </nav>

      <div className="sidebar-footer">
        {loggedIn ? (
          <button className="sidebar-logout-btn" onClick={handleLogout}>
            <IconX size={14} />
            <span>Sign out</span>
          </button>
        ) : (
          <>
            <div className="sidebar-footer-dot" />
            <span>API connected</span>
          </>
        )}
      </div>
    </aside>
  );
}
