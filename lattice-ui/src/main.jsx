import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import AppLite from './AppLite.jsx'

console.log('[Main] Starting render...');

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <AppLite />
  </StrictMode>,
)

console.log('[Main] Render complete');
