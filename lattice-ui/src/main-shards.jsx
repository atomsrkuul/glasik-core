import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import AppWithShards from './AppWithShards.jsx'

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <AppWithShards />
  </StrictMode>,
)
