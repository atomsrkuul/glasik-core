import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import AppDebug from './AppDebug.jsx'

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <AppDebug />
  </StrictMode>,
)
