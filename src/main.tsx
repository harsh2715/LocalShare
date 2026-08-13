import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Activity, ArrowDownToLine, Check, ChevronRight, Clipboard, Copy, FileUp, FolderOpen, HardDrive, Laptop, MessageCircle, MoreHorizontal, Network, Paperclip, Plus, RefreshCw, Search, Send, Settings, ShieldCheck, UploadCloud, Wifi, X } from 'lucide-react';
import './styles.css';

type Peer = { id: string; name: string; address: string; fingerprint: string; state: string; platform: string };
type LocalInfo = { device_id: string; device_name: string; fingerprint: string; port: number; addresses: string[]; mdns: string; transport: string };
type Chat = { peer_id: string; id: string; text: string; sent_at: number; direction: 'sent' | 'received' };
type Transfer = { peer_id: string; id: string; name: string; size: number; transferred: number; state: string; direction: string };

const formatBytes = (n: number) => n < 1024 ? `${n} B` : n < 1024 ** 2 ? `${(n / 1024).toFixed(1)} KB` : n < 1024 ** 3 ? `${(n / 1024 ** 2).toFixed(1)} MB` : `${(n / 1024 ** 3).toFixed(2)} GB`;
const formatTime = (n: number) => new Date(n * 1000).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });

function App() {
  const [info, setInfo] = useState<LocalInfo | null>(null);
  const [peers, setPeers] = useState<Peer[]>([]);
  const [selected, setSelected] = useState<Peer | null>(null);
  const [chats, setChats] = useState<Chat[]>([]);
  const [transfers, setTransfers] = useState<Transfer[]>([]);
  const [message, setMessage] = useState('');
  const [manualOpen, setManualOpen] = useState(false);
  const [manualIp, setManualIp] = useState('');
  const [manualPort, setManualPort] = useState('32123');
  const [notice, setNotice] = useState('');

  const refresh = async () => { try { setInfo(await invoke('local_info')); setPeers(await invoke('list_peers')); } catch (e) { setNotice(String(e)); } };
  useEffect(() => { refresh(); const timer = setInterval(refresh, 3000); const unsubs = [window.__TAURI_INTERNALS__ && listen('peer-updated', (p: Peer) => { setPeers(old => [...old.filter(x => x.id !== p.id), p]); setSelected(current => current?.id === p.id ? p : current); }), listen('chat-message', (c: Chat) => setChats(old => [...old, c])), listen('transfer-progress', (t: Transfer) => setTransfers(old => [...old.filter(x => x.id !== t.id || x.name !== t.name), t])), listen('transfer-complete', (t: Transfer) => setTransfers(old => [...old.filter(x => x.id !== t.id || x.name !== t.name), { ...t, state: 'completed' }]))]; return () => { clearInterval(timer); void unsubs; }; }, []);
  const connectedPeers = useMemo(() => peers.filter(p => p.state === 'connected'), [peers]);
  const activeTransfers = transfers.filter(t => t.state === 'transferring');

  const sendMessage = async () => { if (!selected || !message.trim()) return; try { const id = await invoke<string>('send_chat', { peerId: selected.id, text: message }); setChats(old => [...old, { peer_id: selected.id, id, text: message, sent_at: Date.now() / 1000, direction: 'sent' }]); setMessage(''); } catch (e) { setNotice(String(e)); } };
  const selectFiles = async () => { if (!selected) return; const picked = await open({ multiple: true, directory: false }); if (!picked) return; const paths = Array.isArray(picked) ? picked : [picked]; try { await invoke('send_files', { peerId: selected.id, paths }); } catch (e) { setNotice(String(e)); } };
  const connectManual = async () => { try { await invoke('connect_manual', { ip: manualIp, port: Number(manualPort) }); setNotice('Connection attempt started'); setManualOpen(false); } catch (e) { setNotice(String(e)); } };
  const openPeer = async (peer: Peer) => { setSelected(peer); if (peer.state !== 'connected') { const split = peer.address.lastIndexOf(':'); const ip = peer.address.slice(0, split).replace(/^\[/, '').replace(/\]$/, ''); const port = Number(peer.address.slice(split + 1)); try { await invoke('connect_manual', { ip, port }); setNotice(`Connecting securely to ${peer.name}…`); } catch (e) { setNotice(String(e)); } } };

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><UploadCloud size={21}/></div><span>LocalShare</span><span className="beta">LAN</span></div>
      <div className="sidebar-section-label">WORKSPACE</div>
      <button className="nav-item active"><Network size={17}/> Nearby devices <span className="nav-count">{connectedPeers.length}</span></button>
      <button className="nav-item"><MessageCircle size={17}/> Messages</button>
      <button className="nav-item"><HardDrive size={17}/> Transfer history</button>
      <div className="sidebar-spacer" />
      <div className="secure-card"><ShieldCheck size={18}/><div><strong>Private by design</strong><span>Direct encrypted LAN</span></div></div>
      <button className="nav-item"><Settings size={17}/> Settings</button>
      <button className="nav-item"><Activity size={17}/> Diagnostics</button>
      <div className="version">LocalShare v0.1.0</div>
    </aside>
    <main className="main-content">
      <header className="topbar"><div><h1>Nearby devices</h1><p>Send files and messages across your local network.</p></div><button className="icon-button" onClick={refresh} title="Refresh"><RefreshCw size={18}/></button></header>
      <section className="network-strip"><div className="network-icon"><Wifi size={19}/></div><div><span className="eyebrow">YOUR NETWORK</span><strong>Local network only</strong><span className="subtle">No internet or cloud connection required</span></div><div className="strip-status"><span className="dot green"/> Listening on {info?.addresses?.[0] ?? 'local address'}:{info?.port ?? 32123}</div></section>
      {notice && <div className="notice">{notice}<button onClick={() => setNotice('')}><X size={15}/></button></div>}
      <div className="content-grid">
        <section className="devices-panel"><div className="section-heading"><div><h2>Devices on your network</h2><span className="muted">{peers.length ? `${peers.length} discovered` : 'Searching automatically…'}</span></div><button className="secondary-button" onClick={() => setManualOpen(true)}><Plus size={16}/> Connect manually</button></div>
          <div className="device-list">{peers.length === 0 ? <div className="empty-state"><div className="radar"><Search size={24}/></div><strong>Looking for nearby devices</strong><span>Make sure both computers are on the same Wi-Fi or Ethernet network.</span><button className="link-button" onClick={() => setManualOpen(true)}>Enter an IP address instead <ChevronRight size={14}/></button></div> : peers.map(peer => <button key={peer.id} className={`device-card ${selected?.id === peer.id ? 'selected' : ''}`} onClick={() => void openPeer(peer)}><div className="device-avatar"><Laptop size={22}/></div><div className="device-copy"><strong>{peer.name}</strong><span>{peer.platform} · {peer.address}</span></div><span className={`status-pill ${peer.state}`}><span className="dot"/>{peer.state === 'connected' ? 'Connected securely' : 'Available'}</span><ChevronRight size={17} className="chevron"/></button>)}</div>
          <div className="your-device"><div className="device-avatar self"><Laptop size={20}/></div><div className="device-copy"><span className="eyebrow">THIS DEVICE</span><strong>{info?.device_name ?? 'This device'}</strong><span>{info?.addresses?.[0] ?? 'Detecting address'} · Port {info?.port ?? 32123}</span></div><span className="self-badge"><Check size={13}/> Ready</span></div>
        </section>
        {selected ? <section className="conversation"><div className="conversation-head"><div className="peer-title"><div className="device-avatar small"><Laptop size={18}/></div><div><h2>{selected.name}</h2><span><span className="dot green"/> {selected.state === 'connected' ? 'Connected securely' : 'Available'} · {selected.address}</span></div></div><button className="icon-button"><MoreHorizontal size={19}/></button></div>
          <div className="chat-area">{chats.filter(c => c.peer_id === selected.id).length === 0 ? <div className="chat-empty"><div className="chat-empty-icon"><MessageCircle size={22}/></div><strong>Start a conversation</strong><span>Messages stay on your devices and travel directly over the LAN.</span></div> : chats.filter(c => c.peer_id === selected.id).map(c => <div key={c.id} className={`message-row ${c.direction}`}><div className="bubble">{c.text}<small>{formatTime(c.sent_at)} {c.direction === 'sent' && <Check size={12}/>}</small></div></div>)}{activeTransfers.filter(t => t.peer_id === selected.id).map(t => <div className="inline-transfer" key={t.id + t.name}><div className="inline-transfer-top"><FileUp size={16}/><strong>{t.name}</strong><span>{Math.round((t.transferred / t.size) * 100)}%</span></div><div className="progress"><i style={{ width: `${Math.min(100, t.transferred / t.size * 100)}%` }}/></div><span>{formatBytes(t.transferred)} of {formatBytes(t.size)}</span></div>)}</div>
          <div className="composer"><textarea value={message} onChange={e => setMessage(e.target.value)} onKeyDown={e => { if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') void sendMessage(); }} placeholder="Write a message…" rows={2}/><div className="composer-actions"><button className="icon-button" onClick={selectFiles} title="Send files"><Paperclip size={18}/></button><span className="composer-hint">⌘ + Enter to send</span><button className="send-button" onClick={sendMessage}><Send size={16}/> Send</button></div></div>
        </section> : <section className="welcome-panel"><div className="welcome-art"><div className="art-ring ring-a"/><div className="art-ring ring-b"/><div className="art-center"><UploadCloud size={30}/></div></div><h2>Share with people nearby</h2><p>Select a device to start chatting or sending files. Everything stays on your local network.</p><button className="secondary-button" onClick={() => setManualOpen(true)}><Plus size={16}/> Connect an IP manually</button></section>}
      </div>
      <footer className="footer-bar"><span><ShieldCheck size={14}/> End-to-end encrypted session</span><span><Network size={14}/> Direct peer connection</span><span className="footer-fingerprint">Identity {info?.fingerprint?.slice(0, 12) ?? '—'}…</span></footer>
    </main>
    {manualOpen && <div className="modal-backdrop"><div className="modal"><div className="modal-head"><div><h2>Connect manually</h2><p>Use the address shown in LocalShare on the other computer.</p></div><button className="icon-button" onClick={() => setManualOpen(false)}><X size={18}/></button></div><label>IP address<input value={manualIp} onChange={e => setManualIp(e.target.value)} placeholder="192.168.1.42" autoFocus/></label><label>Port<input value={manualPort} onChange={e => setManualPort(e.target.value)} placeholder="32123"/></label><div className="modal-note"><ShieldCheck size={16}/> The first connection will show a security fingerprint for trust confirmation.</div><div className="modal-actions"><button className="secondary-button" onClick={() => setManualOpen(false)}>Cancel</button><button className="send-button" onClick={connectManual}>Connect <ChevronRight size={16}/></button></div></div></div>}
  </div>;
}

async function listen(event: string, handler: (payload: any) => void) { try { const { listen } = await import('@tauri-apps/api/event'); return listen(event, e => handler(e.payload)); } catch { return undefined; } }
// Tauri exposes this during runtime; keeping the fallback makes the browser preview usable.
declare global { interface Window { __TAURI_INTERNALS__?: unknown } }
createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
