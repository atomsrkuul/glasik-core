import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import AppWithTabs from './AppWithTabs.jsx'

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <AppWithTabs />
  </StrictMode>,
)
