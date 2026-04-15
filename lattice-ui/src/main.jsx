import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import AppMultiTab from './AppMultiTab.jsx'

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <AppMultiTab />
  </StrictMode>,
)
