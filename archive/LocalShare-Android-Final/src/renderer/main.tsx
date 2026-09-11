import React from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { createApi } from './bridge';
import './styles.css';
void createApi().then((api) => { window.localShare = api; createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>); });
