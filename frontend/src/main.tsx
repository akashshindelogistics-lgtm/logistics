import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { ThemeProvider } from './lib/theme.tsx'
import MotionProvider from './lib/MotionProvider.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ThemeProvider>
      <MotionProvider>
        <App />
      </MotionProvider>
    </ThemeProvider>
  </StrictMode>,
)
