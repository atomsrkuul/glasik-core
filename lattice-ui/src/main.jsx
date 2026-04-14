import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import AppWithMultiDB from './AppWithMultiDB.jsx'

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <AppWithMultiDB />
  </StrictMode>,
)
