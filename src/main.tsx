import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';
import {
  Activity,
  AlertCircle,
  ArrowDownToLine,
  ArrowUpFromLine,
  Check,
  CheckCircle2,
  ChevronRight,
  CircleHelp,
  Copy,
  FileUp,
  FolderOpen,
  HardDrive,
  Info,
  Laptop,
  LoaderCircle,
  MessageCircle,
  Network,
  Paperclip,
  Plus,
  RefreshCw,
  Search,
  Send,
  Settings,
  ShieldCheck,
  UploadCloud,
  Wifi,
  X,
} from 'lucide-react';
import './styles.css';

type Peer = {
  id: string;
  name: string;
  address: string;
  fingerprint: string;
  state: string;
  platform: string;
  trusted: boolean;
};
type KnownPeer = { id: string; name: string; fingerprint: string; trusted_at: number };
type LocalInfo = {
  version: string;
  device_id: string;
  device_name: string;
  platform: string;
  architecture: string;
  fingerprint: string;
  port: number;
  addresses: string[];
  mdns: string;
  mdns_advertise: string;
  mdns_browse: string;
  listener_state: string;
  tls_state: string;
  database_state: string;
  receive_directory: string;
  updater: string;
  transport: string;
};
type Chat = {
  peer_id: string;
  id: string;
  text: string;
  sent_at: number;
  direction: 'sent' | 'received';
  state?: string;
};
type ChatRecord = Omit<Chat, 'text'> & { content: string };
type Transfer = {
  peer_id: string;
  id: string;
  name: string;
  size: number;
  transferred: number;
  state: string;
  direction: 'sent' | 'received';
  path?: string;
};
type TransferPatch = Partial<Transfer> & Pick<Transfer, 'id' | 'peer_id'>;
type History = {
  id: string;
  peer_id: string;
  name: string;
  size: number;
  direction: 'sent' | 'received';
  result: string;
  path?: string;
  created_at: number;
};
type SettingsInfo = {
  device_name: string;
  download_location: string;
  ask_before_receive: boolean;
  notifications: boolean;
  check_for_updates: boolean;
};
type Page = 'nearby' | 'messages' | 'history' | 'settings' | 'diagnostics';
type Notice = { message: string; kind: 'success' | 'error' | 'info' };

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const IS_TAURI = Boolean(window.__TAURI_INTERNALS__);
const IS_ANDROID = /Android/i.test(navigator.userAgent);
const MAX_MESSAGE_LENGTH = 16_000;
const EMPTY_SETTINGS: SettingsInfo = {
  device_name: 'This device',
  download_location: 'Downloads/LocalShare',
  ask_before_receive: true,
  notifications: true,
  check_for_updates: true,
};
const PREVIEW_INFO: LocalInfo = {
  version: '0.1.0',
  device_id: 'desktop-preview',
  device_name: 'This device',
  platform: 'desktop',
  architecture: '—',
  fingerprint: 'Open the desktop app to view your identity',
  port: 32123,
  addresses: [],
  mdns: 'preview',
  mdns_advertise: 'preview',
  mdns_browse: 'preview',
  listener_state: 'preview',
  tls_state: 'configured',
  database_state: 'preview',
  receive_directory: 'Downloads/LocalShare',
  updater: 'desktop app only',
  transport: 'TLS 1.3 / direct LAN TCP',
};

const formatBytes = (value: number) => {
  const bytes = Number.isFinite(value) && value > 0 ? value : 0;
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
};
const formatTime = (seconds: number) => {
  if (!Number.isFinite(seconds)) return '';
  return new Date(seconds * 1000).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
};
const formatDateTime = (seconds: number) => {
  if (!Number.isFinite(seconds)) return '';
  const date = new Date(seconds * 1000);
  const today = new Date();
  if (date.toDateString() === today.toDateString()) return formatTime(seconds);
  return date.toLocaleDateString([], { day: 'numeric', month: 'short', year: date.getFullYear() === today.getFullYear() ? undefined : 'numeric' });
};
const safeIp = (value: string) => (value.includes(':') ? `[${value}]` : value);
const groupFingerprint = (value: string) => /^[a-f0-9]+$/i.test(value) ? value.match(/.{1,4}/g)?.join(' ') ?? value : value;
const peerStateLabel = (state: string) => ({
  connected: 'Connected',
  connecting: 'Connecting…',
  trust_required: 'Approval needed',
  discovered: 'Available',
  offline: 'Offline',
}[state] ?? state.replaceAll('_', ' '));
const transferStateLabel = (state: string) => ({
  waiting: 'Waiting for approval',
  pending: 'Waiting for you',
  transferring: 'Transferring',
  saved: 'Saving securely',
  completed: 'Completed',
  cancelled: 'Cancelled',
  rejected: 'Declined',
  failed: 'Failed',
}[state] ?? state);
const friendlyError = (error: unknown) => {
  const raw = error instanceof Error ? error.message : String(error);
  if (raw.includes("reading 'invoke'") || raw.includes('window.__TAURI_INTERNALS__')) return 'Sharing features are available in the LocalShare desktop app.';
  if (raw.includes('Peer is not connected')) return 'That device is offline. Wait for it to reconnect and try again.';
  if (raw.includes('connection closed')) return 'The connection was interrupted. LocalShare will try to reconnect.';
  if (raw.includes('timed out')) return 'The other device did not respond in time. Please try again.';
  return raw.replace(/^Error:\s*/i, '') || 'Something went wrong. Please try again.';
};
const mergeChats = (current: Chat[], incoming: Chat[]) => {
  const byId = new Map(current.map((chat) => [chat.id, chat]));
  incoming.forEach((chat) => byId.set(chat.id, chat));
  return [...byId.values()].sort((a, b) => a.sent_at - b.sent_at);
};
const rowsToChats = (rows: ChatRecord[]): Chat[] => rows.map((row) => ({
  peer_id: row.peer_id,
  id: row.id,
  text: row.content,
  sent_at: row.sent_at,
  direction: row.direction,
  state: row.state,
}));
const copyText = async (value: string) => {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }
  const area = document.createElement('textarea');
  area.value = value;
  area.style.position = 'fixed';
  area.style.opacity = '0';
  document.body.append(area);
  area.select();
  document.execCommand('copy');
  area.remove();
};

async function listenTo<T>(event: string, handler: (payload: T) => void) {
  if (!IS_TAURI) return undefined;
  const { listen } = await import('@tauri-apps/api/event');
  return listen<T>(event, ({ payload }) => handler(payload));
}

function App() {
  const [page, setPage] = useState<Page>('nearby');
  const [info, setInfo] = useState<LocalInfo | null>(null);
  const [peers, setPeers] = useState<Peer[]>([]);
  const [knownPeers, setKnownPeers] = useState<KnownPeer[]>([]);
  const [selected, setSelected] = useState<Peer | null>(null);
  const [chats, setChats] = useState<Chat[]>([]);
  const [transfers, setTransfers] = useState<Transfer[]>([]);
  const [history, setHistory] = useState<History[]>([]);
  const [settings, setSettings] = useState<SettingsInfo | null>(null);
  const [message, setMessage] = useState('');
  const [manualOpen, setManualOpen] = useState(false);
  const [manualIp, setManualIp] = useState('');
  const [manualPort, setManualPort] = useState('32123');
  const [manualError, setManualError] = useState('');
  const [manualConnecting, setManualConnecting] = useState(false);
  const [incomingRequests, setIncomingRequests] = useState<Transfer[]>([]);
  const [trustRequests, setTrustRequests] = useState<Peer[]>([]);
  const [modalBusy, setModalBusy] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [pageLoading, setPageLoading] = useState(false);
  const [sendingMessage, setSendingMessage] = useState(false);
  const [choosingFiles, setChoosingFiles] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [updateState, setUpdateState] = useState<'idle' | 'checking' | 'available' | 'current' | 'offline' | 'installing'>('idle');
  const [updateVersion, setUpdateVersion] = useState('');
  const [notificationPermission, setNotificationPermission] = useState<'unknown' | 'granted' | 'denied'>('unknown');

  const transfersRef = useRef<Transfer[]>([]);
  const selectedRef = useRef<Peer | null>(null);
  const settingsRef = useRef<SettingsInfo | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  useEffect(() => { transfersRef.current = transfers; }, [transfers]);
  useEffect(() => { selectedRef.current = selected; }, [selected]);
  useEffect(() => { settingsRef.current = settings; }, [settings]);
  useEffect(() => { window.scrollTo({ top: 0, behavior: 'auto' }); }, [page]);

  const showNotice = useCallback((messageText: string, kind: Notice['kind'] = 'info') => setNotice({ message: messageText, kind }), []);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), notice.kind === 'error' ? 9_000 : 5_000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const refresh = useCallback(async (quiet = false) => {
    if (!IS_TAURI) {
      setInfo(PREVIEW_INFO);
      setSettings((current) => current ?? EMPTY_SETTINGS);
      return;
    }
    if (!quiet) setRefreshing(true);
    try {
      const [local, nearby, trusted] = await Promise.allSettled([
        invoke<LocalInfo>('local_info'),
        invoke<Peer[]>('list_peers'),
        invoke<KnownPeer[]>('known_peers'),
      ]);
      if (local.status === 'fulfilled') setInfo(local.value);
      if (nearby.status === 'fulfilled') setPeers(nearby.value);
      if (trusted.status === 'fulfilled') setKnownPeers(trusted.value);
      const failure = [local, nearby, trusted].find((result) => result.status === 'rejected');
      if (!quiet && failure?.status === 'rejected') showNotice(friendlyError(failure.reason), 'error');
    } finally {
      if (!quiet) setRefreshing(false);
    }
  }, [showNotice]);

  const loadPage = useCallback(async (next: Page) => {
    setPage(next);
    if (!IS_TAURI) return;
    setPageLoading(true);
    try {
      if (next === 'messages') {
        const [rows, trusted] = await Promise.all([invoke<ChatRecord[]>('chat_history', { peerId: null }), invoke<KnownPeer[]>('known_peers')]);
        setChats((current) => mergeChats(current, rowsToChats(rows)));
        setKnownPeers(trusted);
      }
      if (next === 'history') setHistory(await invoke<History[]>('transfer_history'));
      if (next === 'settings') {
        const [nextSettings, trusted] = await Promise.all([invoke<SettingsInfo>('settings'), invoke<KnownPeer[]>('known_peers')]);
        setSettings(nextSettings);
        setKnownPeers(trusted);
      }
    } catch (error) {
      showNotice(friendlyError(error), 'error');
    } finally {
      setPageLoading(false);
    }
  }, [showNotice]);

  const updateTransfer = useCallback((patch: TransferPatch, forcedState?: string) => {
    const current = transfersRef.current.find((item) => item.id === patch.id);
    if (!current && (!patch.name || !patch.direction)) return undefined;
    const merged = { ...current, ...patch, state: forcedState ?? patch.state ?? current?.state ?? 'waiting' } as Transfer;
    const next = [...transfersRef.current.filter((item) => item.id !== patch.id), merged];
    transfersRef.current = next;
    setTransfers(next);
    return merged;
  }, []);
  const removeTransferLater = useCallback((id: string) => {
    window.setTimeout(() => {
      const next = transfersRef.current.filter((item) => item.id !== id);
      transfersRef.current = next;
      setTransfers(next);
    }, 8_000);
  }, []);
  const notifyTransferComplete = useCallback(async (transfer: Transfer) => {
    if (!IS_TAURI || transfer.direction !== 'received' || !settingsRef.current?.notifications) return;
    try {
      const { isPermissionGranted, sendNotification } = await import('@tauri-apps/plugin-notification');
      if (await isPermissionGranted()) sendNotification({ title: 'File received', body: `${transfer.name} is ready in LocalShare.` });
    } catch {
      // A transfer must never fail because desktop notifications are unavailable.
    }
  }, []);

  useEffect(() => {
    void refresh(true);
    if (!IS_TAURI) return;
    const timer = window.setInterval(() => void refresh(true), 5_000);
    let disposed = false;
    let unsubs: Array<() => void> = [];
    void Promise.all([
      listenTo<Peer>('peer-updated', (peer) => {
        setPeers((current) => [...current.filter((item) => item.id !== peer.id), peer]);
        setSelected((current) => current?.id === peer.id ? peer : current);
        if (peer.state === 'trust_required' && !peer.trusted) setTrustRequests((current) => current.some((item) => item.id === peer.id) ? current : [...current, peer]);
      }),
      listenTo<string>('peer-removed', (id) => {
        setPeers((current) => current.filter((peer) => peer.id !== id));
        setSelected((current) => current?.id === id ? { ...current, state: 'offline' } : current);
      }),
      listenTo<Chat>('chat-message', (chat) => setChats((current) => mergeChats(current, [chat]))),
      listenTo<Transfer>('transfer-request', (transfer) => setIncomingRequests((current) => current.some((item) => item.id === transfer.id) ? current : [...current, transfer])),
      listenTo<Transfer>('transfer-progress', (transfer) => { updateTransfer(transfer); }),
      listenTo<TransferPatch>('transfer-complete', (patch) => {
        const completed = updateTransfer(patch, 'completed');
        if (!completed) return;
        void notifyTransferComplete(completed);
        showNotice(completed.direction === 'received' ? `${completed.name} received successfully.` : 'Transfer completed.', 'success');
        void invoke<History[]>('transfer_history').then(setHistory).catch(() => undefined);
        removeTransferLater(completed.id);
      }),
      listenTo<TransferPatch & { message?: string }>('transfer-error', (patch) => {
        const failed = updateTransfer(patch, patch.state ?? 'failed');
        showNotice(patch.message ?? (failed?.state === 'cancelled' ? 'Transfer cancelled.' : 'Transfer failed.'), failed?.state === 'cancelled' ? 'info' : 'error');
        void invoke<History[]>('transfer_history').then(setHistory).catch(() => undefined);
        if (failed) removeTransferLater(failed.id);
      }),
    ]).then((listeners) => {
      const active = listeners.filter((listener): listener is () => void => Boolean(listener));
      if (disposed) active.forEach((listener) => listener()); else unsubs = active;
    }).catch((error) => showNotice(friendlyError(error), 'error'));
    return () => {
      disposed = true;
      window.clearInterval(timer);
      unsubs.forEach((listener) => listener());
    };
  }, [notifyTransferComplete, refresh, removeTransferLater, showNotice, updateTransfer]);

  useEffect(() => {
    if (!IS_TAURI) return;
    let unlisten: (() => void) | undefined;
    void import('@tauri-apps/api/webview').then(async ({ getCurrentWebview }) => {
      unlisten = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === 'enter' || event.payload.type === 'over') setDragging(true);
        if (event.payload.type === 'leave') setDragging(false);
        if (event.payload.type !== 'drop') return;
        setDragging(false);
        const peer = selectedRef.current;
        if (!peer || peer.state !== 'connected') {
          showNotice('Select a connected device before dropping files.', 'error');
          return;
        }
        if (transfersRef.current.some((item) => item.peer_id === peer.id && ['waiting', 'pending', 'transferring', 'saved'].includes(item.state))) {
          showNotice('Wait for the current transfer to finish, then try again.', 'info');
          return;
        }
        if (!event.payload.paths.length) return;
        void invoke('send_files', { peerId: peer.id, paths: event.payload.paths })
          .then(() => showNotice('Files are waiting for the other device to accept.', 'success'))
          .catch((error) => showNotice(friendlyError(error), 'error'));
      });
    }).catch(() => undefined);
    return () => unlisten?.();
  }, [showNotice]);

  useEffect(() => {
    if (!IS_TAURI) return;
    void invoke<SettingsInfo>('settings').then((value) => {
      setSettings(value);
      if (value.check_for_updates) void checkForUpdates(true);
    }).catch(() => undefined);
    void import('@tauri-apps/plugin-notification').then(async ({ isPermissionGranted }) => {
      setNotificationPermission(await isPermissionGranted() ? 'granted' : 'unknown');
    }).catch(() => setNotificationPermission('denied'));
    // This launch-only check intentionally runs once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const connectedPeers = useMemo(() => peers.filter((peer) => peer.state === 'connected'), [peers]);
  const sortedPeers = useMemo(() => [...peers].sort((a, b) => {
    const rank: Record<string, number> = { connected: 0, trust_required: 1, connecting: 2, discovered: 3, offline: 4 };
    return (rank[a.state] ?? 9) - (rank[b.state] ?? 9) || a.name.localeCompare(b.name);
  }), [peers]);
  const incomingRequest = incomingRequests[0] ?? null;
  const trustRequest = trustRequests[0] ?? null;

  const openPeer = useCallback(async (peer: Peer, navigate = false) => {
    setSelected(peer);
    if (navigate) setPage('nearby');
    if (!IS_TAURI) return;
    try {
      const rows = await invoke<ChatRecord[]>('chat_history', { peerId: peer.id });
      setChats((current) => mergeChats(current, rowsToChats(rows)));
    } catch (error) {
      showNotice(friendlyError(error), 'error');
    }
  }, [showNotice]);

  const sendMessage = async () => {
    const text = message.trim();
    if (!selected || selected.state !== 'connected' || !text || text.length > MAX_MESSAGE_LENGTH) return;
    setSendingMessage(true);
    try {
      if (!IS_TAURI) throw new Error('Sharing features are available in the LocalShare desktop app.');
      const id = await invoke<string>('send_chat', { peerId: selected.id, text });
      setChats((current) => mergeChats(current, [{ peer_id: selected.id, id, text, sent_at: Date.now() / 1000, direction: 'sent', state: 'sent' }]));
      setMessage('');
    } catch (error) {
      showNotice(friendlyError(error), 'error');
    } finally {
      setSendingMessage(false);
    }
  };

  const sendHtmlFiles = async (fileList: FileList | null) => {
    if (!fileList || fileList.length === 0 || !selected || selected.state !== 'connected') return;
    setChoosingFiles(true);
    showNotice(`Preparing ${fileList.length} file(s) for transfer…`, 'info');
    try {
      const cachedPaths: string[] = [];
      const CHUNK_SIZE = 256 * 1024;
      for (let f = 0; f < fileList.length; f++) {
        const file = fileList[f];
        const transferFileId = `${Date.now()}_${f}`;
        let cachedPath: string | null = null;
        const totalChunks = Math.ceil(file.size / CHUNK_SIZE) || 1;
        for (let i = 0; i < totalChunks; i++) {
          const start = i * CHUNK_SIZE;
          const end = Math.min(file.size, start + CHUNK_SIZE);
          const slice = file.slice(start, end);
          const base64 = await new Promise<string>((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => {
              const res = reader.result as string;
              resolve(res.split(',')[1] || '');
            };
            reader.onerror = reject;
            reader.readAsDataURL(slice);
          });
          const res = await invoke<string | null>('append_upload_chunk', {
            transferFileId,
            name: file.name,
            chunkBase64: base64,
            isFirst: i === 0,
            isLast: i === totalChunks - 1,
          });
          if (res) cachedPath = res;
        }
        if (cachedPath) cachedPaths.push(cachedPath);
      }
      if (cachedPaths.length > 0) {
        await invoke('send_files', { peerId: selected.id, paths: cachedPaths });
        showNotice('Files sent! Transfer in progress…', 'success');
      }
    } catch (error) {
      showNotice(friendlyError(error), 'error');
    } finally {
      setChoosingFiles(false);
      if (fileInputRef.current) fileInputRef.current.value = '';
      if (folderInputRef.current) folderInputRef.current.value = '';
    }
  };

  const selectFiles = async () => {
    if (!selected || selected.state !== 'connected') return;
    if (IS_ANDROID) {
      fileInputRef.current?.click();
      return;
    }
    setChoosingFiles(true);
    try {
      const picked = await open({ multiple: true, directory: false });
      if (!picked) return;
      const paths = Array.isArray(picked) ? picked : [picked];
      await invoke('send_files', { peerId: selected.id, paths });
      showNotice('Files sent! Transfer in progress…', 'success');
    } catch {
      fileInputRef.current?.click();
    } finally {
      setChoosingFiles(false);
    }
  };

  const selectFolder = async () => {
    if (!selected || selected.state !== 'connected') return;
    if (IS_ANDROID) {
      fileInputRef.current?.click();
      return;
    }
    setChoosingFiles(true);
    try {
      const picked = await open({ directory: true, multiple: false });
      if (!picked || Array.isArray(picked)) return;
      await invoke('send_files', { peerId: selected.id, paths: [picked] });
      showNotice('Folder sent! Transfer in progress…', 'success');
    } catch {
      folderInputRef.current?.click();
    } finally {
      setChoosingFiles(false);
    }
  };

  const cancelTransfer = async (id: string) => {
    try {
      await invoke('cancel_transfer', { transferId: id });
      updateTransfer({ id, peer_id: selected?.id ?? '' }, 'cancelled');
    } catch (error) {
      showNotice(friendlyError(error), 'error');
    }
  };

  const respondToTransfer = async (accept: boolean) => {
    if (!incomingRequest) return;
    setModalBusy(true);
    try {
      await invoke(accept ? 'accept_transfer' : 'reject_transfer', { transferId: incomingRequest.id });
      if (accept) {
        updateTransfer({ ...incomingRequest, state: 'waiting' });
        showNotice('Transfer accepted. Keep LocalShare open while it finishes.', 'success');
      } else showNotice('Transfer declined.', 'info');
    } catch (error) {
      showNotice(friendlyError(error), 'error');
    } finally {
      setIncomingRequests((current) => current.filter((item) => item.id !== incomingRequest.id));
      setModalBusy(false);
    }
  };

  const respondToTrust = async (accept: boolean) => {
    if (!trustRequest) return;
    if (!accept) {
      setTrustRequests((current) => current.filter((item) => item.id !== trustRequest.id));
      return;
    }
    setModalBusy(true);
    try {
      await invoke('trust_peer', { peerId: trustRequest.id });
      showNotice(`${trustRequest.name} approved. Connecting securely…`, 'success');
      setTrustRequests((current) => current.filter((item) => item.id !== trustRequest.id));
      void refresh(true);
    } catch (error) {
      showNotice(friendlyError(error), 'error');
    } finally {
      setModalBusy(false);
    }
  };

  const connectManual = async () => {
    const ip = manualIp.trim().replace(/^\[|\]$/g, '');
    const port = Number(manualPort);
    if (!ip) {
      setManualError('Enter the IP address shown on the other device.');
      return;
    }
    if (!Number.isInteger(port) || port < 1 || port > 65_535) {
      setManualError('Port must be a number between 1 and 65535.');
      return;
    }
    setManualError('');
    setManualConnecting(true);
    try {
      await invoke('connect_manual', { ip, port });
      showNotice('Device found. Complete approval if prompted.', 'success');
      setManualOpen(false);
      setManualIp('');
      void refresh(true);
    } catch (error) {
      setManualError(friendlyError(error));
    } finally {
      setManualConnecting(false);
    }
  };

  async function checkForUpdates(quiet = false) {
    if (IS_ANDROID) {
      setUpdateState('idle');
      if (!quiet) showNotice('Android updates are installed with a new APK.', 'info');
      return;
    }
    if (!IS_TAURI) {
      if (!quiet) showNotice('Update checks are available in the desktop app.', 'info');
      return;
    }
    setUpdateState('checking');
    try {
      const update = await check();
      if (!update) {
        setUpdateState('current');
        if (!quiet) showNotice('LocalShare is up to date.', 'success');
        return;
      }
      setUpdateVersion(update.version);
      setUpdateState('available');
      if (quiet) return;
      const confirmed = window.confirm(`LocalShare ${update.version} is available. Install it now?`);
      if (!confirmed) return;
      setUpdateState('installing');
      let downloaded = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') showNotice(`Downloading LocalShare ${update.version}…`, 'info');
        if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
          showNotice(`Downloading update… ${formatBytes(downloaded)}`, 'info');
        }
        if (event.event === 'Finished') showNotice('Update ready. Restarting LocalShare…', 'success');
      });
      await relaunch();
    } catch {
      setUpdateState('offline');
      if (!quiet) showNotice('Could not check for updates. LAN sharing still works normally.', 'info');
    }
  }

  const chooseDownloadFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (!picked || Array.isArray(picked) || !settings) return;
    setSettings({ ...settings, download_location: picked });
  };

  const saveSettings = async (draft: SettingsInfo) => {
    let effective = draft;
    let permissionDenied = false;
    if (draft.notifications && IS_TAURI) {
      try {
        const { isPermissionGranted, requestPermission } = await import('@tauri-apps/plugin-notification');
        const granted = await isPermissionGranted() || await requestPermission() === 'granted';
        setNotificationPermission(granted ? 'granted' : 'denied');
        if (!granted) {
          permissionDenied = true;
          effective = { ...draft, notifications: false };
        }
      } catch {
        permissionDenied = true;
        setNotificationPermission('denied');
        effective = { ...draft, notifications: false };
      }
    }
    try {
      if (IS_TAURI) await invoke('save_settings', { deviceName: effective.device_name, downloadLocation: effective.download_location, askBeforeReceive: effective.ask_before_receive, notifications: effective.notifications, checkForUpdates: effective.check_for_updates });
      setSettings(effective);
      showNotice(permissionDenied ? 'Settings saved. Notifications remain off because permission was not granted.' : 'Settings saved.', permissionDenied ? 'info' : 'success');
      void refresh(true);
    } catch (error) {
      showNotice(friendlyError(error), 'error');
      throw error;
    }
  };

  const clearHistory = async () => {
    try {
      await invoke('clear_transfer_history');
      setHistory([]);
      setConfirmClear(false);
      showNotice('Transfer history cleared. Your files were not deleted.', 'success');
    } catch (error) {
      showNotice(friendlyError(error), 'error');
    }
  };

  const copyDiagnostics = async () => {
    const text = `LocalShare ${info?.version ?? '—'}\nDevice: ${info?.device_name ?? '—'}\nDevice ID: ${info?.device_id ?? '—'}\nPlatform: ${info?.platform ?? '—'} ${info?.architecture ?? ''}\nLAN: ${info?.addresses?.join(', ') || 'none'}\nPort: ${info?.port ?? '—'}\nListener: ${info?.listener_state ?? '—'}\nmDNS advertise: ${info?.mdns_advertise ?? '—'}\nmDNS browse: ${info?.mdns_browse ?? '—'}\nTLS: ${info?.tls_state ?? '—'}\nDatabase: ${info?.database_state ?? '—'}\nReceive directory: ${info?.receive_directory ?? '—'}\nUpdater: ${info?.updater ?? '—'}\nPeers: ${peers.length}`;
    try {
      await copyText(text);
      showNotice('Diagnostics copied.', 'success');
    } catch {
      showNotice('Could not copy diagnostics. Please try again.', 'error');
    }
  };

  const pageCopy: Record<Page, { title: string; subtitle: string }> = {
    nearby: { title: 'Nearby devices', subtitle: 'Share files, folders and messages on your local network.' },
    messages: { title: 'Messages', subtitle: 'Your conversations are stored only on this device.' },
    history: { title: 'Transfer history', subtitle: 'Find files you sent or received.' },
    settings: { title: 'Settings', subtitle: 'Choose how this device appears and receives files.' },
    diagnostics: { title: 'Diagnostics', subtitle: 'Check LocalShare services and connection details.' },
  };

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><UploadCloud size={21}/></div><span>LocalShare</span><span className="beta">LAN</span></div>
      <div className="sidebar-section-label">SHARE</div>
      <NavButton active={page === 'nearby'} onClick={() => void loadPage('nearby')} icon={<Network size={18}/>} label="Nearby devices" count={connectedPeers.length}/>
      <NavButton active={page === 'messages'} onClick={() => void loadPage('messages')} icon={<MessageCircle size={18}/>} label="Messages"/>
      <NavButton active={page === 'history'} onClick={() => void loadPage('history')} icon={<HardDrive size={18}/>} label="Transfer history" count={transfers.length || undefined}/>
      <div className="sidebar-spacer"/>
      <div className="secure-card"><ShieldCheck size={19}/><div><strong>Private by design</strong><span>No cloud. No account.</span></div></div>
      <NavButton active={page === 'settings'} onClick={() => void loadPage('settings')} icon={<Settings size={18}/>} label="Settings"/>
      <NavButton active={page === 'diagnostics'} onClick={() => void loadPage('diagnostics')} icon={<Activity size={18}/>} label="Diagnostics"/>
      <div className="version">LocalShare v{info?.version ?? '…'}</div>
    </aside>
    <main className="main-content">
      <header className="topbar"><div><h1>{pageCopy[page].title}</h1><p>{pageCopy[page].subtitle}</p></div><button type="button" className="icon-button" onClick={() => void refresh(false)} aria-label="Refresh LocalShare" title="Refresh" disabled={refreshing}><RefreshCw size={18} className={refreshing ? 'spin' : ''}/></button></header>
      {notice && <div className={`notice ${notice.kind}`} role={notice.kind === 'error' ? 'alert' : 'status'}>{notice.kind === 'success' ? <CheckCircle2 size={17}/> : notice.kind === 'error' ? <AlertCircle size={17}/> : <Info size={17}/>}<span>{notice.message}</span><button type="button" onClick={() => setNotice(null)} aria-label="Dismiss message"><X size={16}/></button></div>}
      {pageLoading && <div className="page-loading" role="status"><LoaderCircle size={18} className="spin"/> Loading…</div>}
      {page === 'nearby' && <Nearby info={info} peers={sortedPeers} selected={selected} onOpenPeer={(peer) => void openPeer(peer)} onManual={() => { setManualError(''); setManualOpen(true); }} onDiagnostics={() => void loadPage('diagnostics')} chats={chats} transfers={transfers} message={message} onMessage={setMessage} onSend={() => void sendMessage()} sendingMessage={sendingMessage} choosingFiles={choosingFiles} onFiles={() => void selectFiles()} onFolder={() => void selectFolder()} onCancel={(id) => void cancelTransfer(id)} onTrust={(peer) => setTrustRequests((current) => current.some((item) => item.id === peer.id) ? current : [...current, peer])} onAccept={(id) => { const req = incomingRequests.find((r) => r.id === id); if (req) { void respondToTransfer(true); } else { void invoke('accept_transfer', { transferId: id }).then(() => { updateTransfer({ id, peer_id: selected?.id ?? '', state: 'waiting' }); }).catch((e) => showNotice(friendlyError(e), 'error')); } }} onReject={(id) => { const req = incomingRequests.find((r) => r.id === id); if (req) { void respondToTransfer(false); } else { void invoke('reject_transfer', { transferId: id }).then(() => { updateTransfer({ id, peer_id: selected?.id ?? '', state: 'rejected' }); }).catch((e) => showNotice(friendlyError(e), 'error')); } }}/>}
      {page === 'messages' && <Messages peers={peers} knownPeers={knownPeers} chats={chats} onSelect={(peer) => void openPeer(peer, true)}/>}
      {page === 'history' && <HistoryPage rows={history} peers={peers} knownPeers={knownPeers} onClear={() => setConfirmClear(true)} onOpenFolder={(path) => void invoke('open_folder', { path }).catch((error) => showNotice(friendlyError(error), 'error'))} onOpenFile={(path) => void invoke('open_file', { path }).catch((error) => showNotice(friendlyError(error), 'error'))}/>}
      {page === 'settings' && <SettingsPage value={settings} knownPeers={knownPeers} notificationPermission={notificationPermission} updateState={updateState} updateVersion={updateVersion} onSave={saveSettings} onChooseFolder={() => void chooseDownloadFolder()} onOpenFolder={() => settings && void invoke('open_folder', { path: settings.download_location }).catch((error) => showNotice(friendlyError(error), 'error'))} onCheckUpdates={() => void checkForUpdates(false)}/>}
      {page === 'diagnostics' && <Diagnostics info={info} peers={peers} knownPeers={knownPeers} onCopy={() => void copyDiagnostics()} onOpenFolder={() => info && void invoke('open_folder', { path: info.receive_directory }).catch((error) => showNotice(friendlyError(error), 'error'))}/>}
      <footer className="footer-bar"><span><ShieldCheck size={14}/> TLS-encrypted</span><span><Network size={14}/> Direct LAN connection</span><span className="footer-fingerprint">Identity {info?.fingerprint && info.fingerprint.length > 16 ? `${info.fingerprint.slice(0, 12)}…` : '—'}</span></footer>
    </main>
    {dragging && <div className="drop-overlay" role="status"><UploadCloud size={36}/><strong>Drop to send</strong><span>{selected?.state === 'connected' ? `Files will be sent to ${selected.name}` : 'Select a connected device first'}</span></div>}
    {manualOpen && <Modal title="Connect using IP" description="Use this only when automatic discovery does not find the other device." onClose={() => setManualOpen(false)}><label className="field">IP address<input value={manualIp} onChange={(event) => { setManualIp(event.target.value); setManualError(''); }} placeholder="192.168.1.42 or 2001:db8::1" autoFocus inputMode="text"/></label><label className="field">Port<input value={manualPort} onChange={(event) => { setManualPort(event.target.value); setManualError(''); }} placeholder="32123" inputMode="numeric"/></label>{manualError && <div className="field-error" role="alert"><AlertCircle size={15}/>{manualError}</div>}<div className="modal-note"><CircleHelp size={17}/><span>On the other computer, open <strong>Diagnostics</strong> and copy its LAN address and port.</span></div><div className="modal-actions"><button type="button" className="secondary-button" onClick={() => setManualOpen(false)}>Cancel</button><button type="button" className="primary-button" onClick={() => void connectManual()} disabled={manualConnecting}>{manualConnecting ? <LoaderCircle size={16} className="spin"/> : <ChevronRight size={16}/>} Connect</button></div></Modal>}
    {incomingRequest && <Modal title="Incoming transfer" description={`${incomingRequest.name} · ${formatBytes(incomingRequest.size)}`} onClose={() => void respondToTransfer(false)}><div className="transfer-review"><div className="review-icon"><ArrowDownToLine size={23}/></div><div><strong>{incomingRequest.name}</strong><span>{formatBytes(incomingRequest.size)} from a trusted device</span></div></div><div className="modal-note"><ShieldCheck size={17}/><span>Files are verified before they appear in your LocalShare downloads folder.</span></div><div className="modal-actions"><button type="button" className="secondary-button" onClick={() => void respondToTransfer(false)} disabled={modalBusy}>Decline</button><button type="button" className="primary-button" onClick={() => void respondToTransfer(true)} disabled={modalBusy}>{modalBusy ? <LoaderCircle size={16} className="spin"/> : <Check size={16}/>} Accept</button></div>{incomingRequests.length > 1 && <div className="queue-note">{incomingRequests.length - 1} more request{incomingRequests.length > 2 ? 's' : ''} waiting</div>}</Modal>}
    {trustRequest && <Modal title="Approve new device" description={`${trustRequest.name} · ${trustRequest.platform}`} onClose={() => void respondToTrust(false)}><div className="trust-copy"><span>Identity fingerprint</span><code>{groupFingerprint(trustRequest.fingerprint)}</code><button type="button" className="copy-button" onClick={() => void copyText(trustRequest.fingerprint).then(() => showNotice('Fingerprint copied.', 'success')).catch(() => showNotice('Could not copy fingerprint.', 'error'))}><Copy size={15}/> Copy</button></div><div className="modal-note warning"><AlertCircle size={17}/><span>Compare this fingerprint with <strong>Diagnostics → Identity</strong> on the other device. Approve only if it matches.</span></div><div className="modal-actions"><button type="button" className="secondary-button" onClick={() => void respondToTrust(false)} disabled={modalBusy}>Not now</button><button type="button" className="primary-button" onClick={() => void respondToTrust(true)} disabled={modalBusy}>{modalBusy ? <LoaderCircle size={16} className="spin"/> : <ShieldCheck size={16}/>} Approve device</button></div>{trustRequests.length > 1 && <div className="queue-note">{trustRequests.length - 1} more device{trustRequests.length > 2 ? 's' : ''} waiting</div>}</Modal>}
    {confirmClear && <Modal title="Clear transfer history?" description="This removes only the activity list, not your sent or received files." onClose={() => setConfirmClear(false)}><div className="modal-note warning"><AlertCircle size={17}/><span>This action cannot be undone.</span></div><div className="modal-actions"><button type="button" className="secondary-button" onClick={() => setConfirmClear(false)}>Keep history</button><button type="button" className="danger-button" onClick={() => void clearHistory()}>Clear history</button></div></Modal>}
    <input
      type="file"
      ref={fileInputRef}
      style={{ display: 'none' }}
      multiple
      onChange={(event) => void sendHtmlFiles(event.target.files)}
    />
    <input
      type="file"
      ref={folderInputRef}
      style={{ display: 'none' }}
      // @ts-expect-error webkitdirectory is standard in browser engines
      webkitdirectory=""
      onChange={(event) => void sendHtmlFiles(event.target.files)}
    />
  </div>;
}

function NavButton({ active, onClick, icon, label, count }: { active: boolean; onClick: () => void; icon: React.ReactNode; label: string; count?: number }) {
  return <button type="button" className={`nav-item ${active ? 'active' : ''}`} onClick={onClick} aria-current={active ? 'page' : undefined}>{icon}<span>{label}</span>{count !== undefined && <span className="nav-count">{count}</span>}</button>;
}

function Modal({ title, description, onClose, children }: { title: string; description: string; onClose: () => void; children: React.ReactNode }) {
  const titleId = `modal-${title.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`;
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === 'Escape') onClose(); };
    window.addEventListener('keydown', closeOnEscape);
    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [onClose]);
  return <div className="modal-backdrop" onMouseDown={(event) => { if (event.currentTarget === event.target) onClose(); }}><section className="modal" role="dialog" aria-modal="true" aria-labelledby={titleId}><div className="modal-head"><div><h2 id={titleId}>{title}</h2><p>{description}</p></div><button type="button" className="icon-button" onClick={onClose} aria-label={`Close ${title}`}><X size={18}/></button></div>{children}</section></div>;
}

type NearbyProps = {
  info: LocalInfo | null;
  peers: Peer[];
  selected: Peer | null;
  onOpenPeer: (peer: Peer) => void;
  onManual: () => void;
  onDiagnostics: () => void;
  chats: Chat[];
  transfers: Transfer[];
  message: string;
  onMessage: (value: string) => void;
  onSend: () => void;
  sendingMessage: boolean;
  choosingFiles: boolean;
  onFiles: () => void;
  onFolder: () => void;
  onCancel: (id: string) => void;
  onTrust: (peer: Peer) => void;
  onAccept?: (id: string) => void;
  onReject?: (id: string) => void;
};

function Nearby(props: NearbyProps) {
  const { info, peers, selected, onOpenPeer, onManual, onDiagnostics } = props;
  const listenerReady = info?.listener_state === 'active';
  const listenerFailed = info?.listener_state === 'failed';
  return <>
    <section className={`network-strip ${listenerFailed ? 'problem' : ''}`}>
      <div className="network-icon">{listenerFailed ? <AlertCircle size={20}/> : <Wifi size={20}/>}</div>
      <div><span className="eyebrow">YOUR NETWORK</span><strong>{listenerFailed ? 'Local sharing needs attention' : 'Local network only'}</strong><span className="subtle">No account, cloud upload or Internet required</span></div>
      <div className="strip-status"><span className={`dot ${listenerReady ? 'green' : listenerFailed ? 'red' : 'amber'}`}/>{info?.addresses?.[0] ? `${safeIp(info.addresses[0])}:${info.port}` : listenerFailed ? 'Listener unavailable' : 'Detecting LAN…'}</div>
      {listenerFailed && <button type="button" className="secondary-button" onClick={onDiagnostics}>View diagnostics</button>}
    </section>
    <div className="content-grid">
      <section className="devices-panel">
        <div className="section-heading"><div><h2>Devices on your network</h2><span className="muted">{peers.length ? `${peers.length} device${peers.length === 1 ? '' : 's'} found` : 'Searching automatically…'}</span></div><button type="button" className="secondary-button" onClick={onManual}><Plus size={16}/> Connect by IP</button></div>
        <div className="device-list">
          {peers.length === 0 ? <GettingStarted onManual={onManual}/> : peers.map((peer) => <button type="button" key={peer.id} className={`device-card ${selected?.id === peer.id ? 'selected' : ''}`} onClick={() => onOpenPeer(peer)}>
            <div className="device-avatar"><Laptop size={22}/></div>
            <div className="device-copy"><strong>{peer.name}</strong><span>{peer.platform}{peer.address ? ` · ${peer.address}` : ''}</span></div>
            <span className={`status-pill ${peer.state}`}><span className="dot"/>{peerStateLabel(peer.state)}</span><ChevronRight size={17} className="chevron"/>
          </button>)}
        </div>
        <div className="your-device"><div className="device-avatar self"><Laptop size={20}/></div><div className="device-copy"><span className="eyebrow">THIS DEVICE</span><strong>{info?.device_name ?? 'Starting LocalShare…'}</strong><span>{info?.addresses?.[0] ? safeIp(info.addresses[0]) : 'Detecting LAN address'} · Port {info?.port ?? '—'}</span></div><span className={`self-badge ${listenerFailed ? 'problem' : ''}`}>{listenerFailed ? <AlertCircle size={13}/> : listenerReady ? <Check size={13}/> : <LoaderCircle size={13} className="spin"/>}{listenerFailed ? 'Needs attention' : listenerReady ? 'Ready' : 'Starting'}</span></div>
      </section>
      {selected ? <Conversation {...props} selected={selected}/> : <section className="welcome-panel"><div className="welcome-art"><div className="art-ring ring-a"/><div className="art-ring ring-b"/><div className="art-center"><UploadCloud size={31}/></div></div><h2>Select a device to start sharing</h2><p>Once connected, you can drag in files, choose a folder, or send a quick message.</p><div className="privacy-points"><span><Check size={14}/> Direct device-to-device transfer</span><span><Check size={14}/> SHA-256 file verification</span><span><Check size={14}/> Encrypted on your local network</span></div></section>}
    </div>
  </>;
}

function GettingStarted({ onManual }: { onManual: () => void }) {
  return <div className="getting-started"><div className="radar"><Search size={24}/><i/></div><strong>Ready when the other device is</strong><span className="empty-copy">LocalShare discovers nearby computers automatically.</span><ol><li><b>1</b><span>Open LocalShare on both computers</span></li><li><b>2</b><span>Connect them to the same Wi-Fi or Ethernet network</span></li><li><b>3</b><span>Approve the identity fingerprint once</span></li></ol><button type="button" className="link-button" onClick={onManual}>Still not showing? Connect by IP <ChevronRight size={14}/></button></div>;
}

function Conversation({ selected, chats, transfers, message, onMessage, onSend, sendingMessage, choosingFiles, onFiles, onFolder, onCancel, onTrust, onAccept, onReject }: NearbyProps & { selected: Peer }) {
  const peerChats = chats.filter((chat) => chat.peer_id === selected.id);
  const peerTransfer = transfers.find((transfer) => transfer.peer_id === selected.id);
  const connected = selected.state === 'connected';
  const transferActive = Boolean(peerTransfer && ['waiting', 'pending', 'transferring', 'saved'].includes(peerTransfer.state));
  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [peerChats.length, peerTransfer?.transferred, peerTransfer?.state]);
  const progress = peerTransfer?.size ? Math.min(100, Math.round(peerTransfer.transferred / peerTransfer.size * 100)) : peerTransfer?.state === 'completed' ? 100 : 0;
  return <section className="conversation">
    <div className="conversation-head"><div className="peer-title"><div className="device-avatar small"><Laptop size={18}/></div><div><h2>{selected.name}</h2><span><span className={`dot ${connected ? 'green' : selected.state === 'trust_required' ? 'amber' : ''}`}/>{connected ? 'Connected and ready' : peerStateLabel(selected.state)}</span></div></div></div>
    {!connected && <div className={`conversation-banner ${selected.state === 'trust_required' ? 'approval' : ''}`}><AlertCircle size={16}/><span>{selected.state === 'trust_required' ? 'Approve this device before sharing files or messages.' : 'This device is offline. Messages and files can be sent after it reconnects.'}</span>{selected.state === 'trust_required' && <button type="button" className="link-button" onClick={() => onTrust(selected)}>Review identity</button>}</div>}
    <div className="chat-area">
      {peerChats.length === 0 && !peerTransfer && <div className="chat-empty"><div className="chat-empty-icon"><MessageCircle size={22}/></div><strong>{connected ? 'You’re connected' : 'No messages yet'}</strong><span>{connected ? 'Send a message or add files below. Everything travels directly over your LAN.' : 'Your existing conversation will stay here when this device reconnects.'}</span></div>}
      {peerChats.map((chat) => <div key={chat.id} className={`message-row ${chat.direction}`}><div className="bubble">{chat.text}<small>{formatTime(chat.sent_at)} {chat.direction === 'sent' && <Check size={12}/>}</small></div></div>)}
      {peerTransfer && <div className={`inline-transfer ${peerTransfer.state}`}><div className="inline-transfer-top"><div className="transfer-file-icon">{peerTransfer.direction === 'received' ? <ArrowDownToLine size={17}/> : <ArrowUpFromLine size={17}/>}</div><div><strong>{peerTransfer.name || 'Transfer'}</strong><span>{transferStateLabel(peerTransfer.state)}</span></div><b>{progress}%</b></div><div className="progress" aria-label={`${progress}% complete`}><i style={{ width: `${progress}%` }}/></div><div className="transfer-meta"><span>{formatBytes(peerTransfer.transferred)} of {formatBytes(peerTransfer.size)}</span>
        {peerTransfer.direction === 'received' && (peerTransfer.state === 'pending' || peerTransfer.state === 'waiting') && (
          <div style={{ display: 'flex', gap: '8px', marginLeft: 'auto' }}>
            <button type="button" className="secondary-button" style={{ padding: '4px 10px', fontSize: '11px' }} onClick={() => onReject?.(peerTransfer.id)}>Decline</button>
            <button type="button" className="primary-button" style={{ padding: '4px 10px', fontSize: '11px' }} onClick={() => onAccept?.(peerTransfer.id)}>Accept</button>
          </div>
        )}
        {transferActive && peerTransfer.direction === 'sent' && <button type="button" className="link-button" onClick={() => onCancel(peerTransfer.id)}>Cancel transfer</button>}
      </div></div>}
      <div ref={bottomRef}/>
    </div>
    <div className="composer"><label className="sr-only" htmlFor="message-composer">Message to {selected.name}</label><textarea id="message-composer" value={message} maxLength={MAX_MESSAGE_LENGTH} onChange={(event) => onMessage(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); onSend(); } }} placeholder={connected ? 'Write a message…' : 'Waiting for this device to reconnect…'} rows={2} disabled={!connected}/><div className="composer-actions"><button type="button" className="icon-button" onClick={onFiles} title="Choose files" aria-label="Choose files" disabled={!connected || transferActive || choosingFiles}>{choosingFiles ? <LoaderCircle size={18} className="spin"/> : <Paperclip size={18}/>}</button><button type="button" className="secondary-button" onClick={onFolder} title="Send a folder" disabled={!connected || transferActive || choosingFiles}><FolderOpen size={16}/> Folder</button><span className="composer-hint">{message.length > MAX_MESSAGE_LENGTH * .9 ? `${message.length.toLocaleString()} / ${MAX_MESSAGE_LENGTH.toLocaleString()}` : 'Enter to send · Shift + Enter for a new line'}</span><button type="button" className="primary-button" onClick={onSend} disabled={!connected || !message.trim() || sendingMessage}>{sendingMessage ? <LoaderCircle size={16} className="spin"/> : <Send size={16}/>} Send</button></div></div>
  </section>;
}

function Messages({ peers, knownPeers, chats, onSelect }: { peers: Peer[]; knownPeers: KnownPeer[]; chats: Chat[]; onSelect: (peer: Peer) => void }) {
  const [query, setQuery] = useState('');
  const peerMap = new Map<string, Peer>();
  knownPeers.forEach((peer) => peerMap.set(peer.id, { ...peer, address: '', platform: 'Previously approved', state: 'offline', trusted: true }));
  peers.forEach((peer) => peerMap.set(peer.id, peer));
  const latest = new Map<string, Chat>();
  chats.forEach((chat) => latest.set(chat.peer_id, chat));
  const rows = [...latest.entries()].map(([id, last]) => ({ peer: peerMap.get(id) ?? { id, name: `Device ${id.slice(0, 8)}`, address: '', fingerprint: '', platform: 'LAN device', state: 'offline', trusted: true }, last })).filter(({ peer }) => (peer.name ?? '').toLowerCase().includes(query.toLowerCase()) || (latest.get(peer.id)?.text ?? '').toLowerCase().includes(query.toLowerCase())).sort((a, b) => b.last.sent_at - a.last.sent_at);
  return <section className="page-card"><div className="section-heading page-heading"><div><h2>Conversations</h2><span className="muted">Available even when a device is offline</span></div>{chats.length > 0 && <label className="search-field"><Search size={15}/><span className="sr-only">Search messages</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search"/></label>}</div>{rows.length === 0 ? <div className="empty-state"><div className="empty-icon"><MessageCircle size={25}/></div><strong>{query ? 'No matching conversations' : 'No conversations yet'}</strong><span>{query ? 'Try a different device name or message.' : 'Select a nearby device and send a message to start.'}</span></div> : <div className="history-list">{rows.map(({ peer, last }) => <button type="button" className="history-row conversation-row" key={peer.id} onClick={() => onSelect(peer)}><div className="device-avatar small"><Laptop size={18}/></div><div className="device-copy"><strong>{peer.name}</strong><span>{last.direction === 'sent' ? 'You: ' : ''}{last.text}</span></div><div className="row-tail"><span className={`connection-label ${peer.state}`}>{peerStateLabel(peer.state)}</span><span className="muted">{formatDateTime(last.sent_at)}</span></div><ChevronRight size={16}/></button>)}</div>}</section>;
}

function HistoryPage({ rows, peers, knownPeers, onClear, onOpenFolder, onOpenFile }: { rows: History[]; peers: Peer[]; knownPeers: KnownPeer[]; onClear: () => void; onOpenFolder: (path: string) => void; onOpenFile: (path: string) => void }) {
  const names = new Map(knownPeers.map((peer) => [peer.id, peer.name]));
  peers.forEach((peer) => names.set(peer.id, peer.name));
  return <section className="page-card"><div className="section-heading page-heading"><div><h2>Recent activity</h2><span className="muted">Clearing this list never deletes your files</span></div>{rows.length > 0 && <button type="button" className="secondary-button" onClick={onClear}>Clear history</button>}</div>{rows.length === 0 ? <div className="empty-state"><div className="empty-icon"><HardDrive size={25}/></div><strong>No transfers yet</strong><span>Completed, cancelled and failed transfers will appear here.</span></div> : <div className="history-list">{rows.map((row) => <div className="history-row" key={row.id}><div className={`device-avatar small ${row.direction}`}>{row.direction === 'sent' ? <ArrowUpFromLine size={17}/> : <ArrowDownToLine size={17}/>}</div><div className="device-copy"><strong>{row.name}</strong><span>{row.direction === 'sent' ? 'Sent to' : 'Received from'} {names.get(row.peer_id) ?? `device ${row.peer_id.slice(0, 8)}`} · {formatBytes(row.size)}</span>{row.path && <small title={row.path}>{row.path}</small>}</div><span className={`result-pill ${row.result}`}>{row.result}</span><span className="muted history-time">{formatDateTime(row.created_at)}</span>{row.path && row.result === 'completed' && <div className="row-actions"><button type="button" className="icon-button" onClick={() => onOpenFile(row.path!)} title="Open file" aria-label={`Open ${row.name}`}><FileUp size={15}/></button><button type="button" className="icon-button" onClick={() => onOpenFolder(row.path!)} title="Show in folder" aria-label={`Show ${row.name} in folder`}><FolderOpen size={15}/></button></div>}</div>)}</div>}</section>;
}

function SettingsPage({ value, knownPeers, notificationPermission, updateState, updateVersion, onSave, onChooseFolder, onOpenFolder, onCheckUpdates }: { value: SettingsInfo | null; knownPeers: KnownPeer[]; notificationPermission: 'unknown' | 'granted' | 'denied'; updateState: string; updateVersion: string; onSave: (value: SettingsInfo) => Promise<void>; onChooseFolder: () => void; onOpenFolder: () => void; onCheckUpdates: () => void }) {
  const [draft, setDraft] = useState<SettingsInfo | null>(value);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    setDraft(value);
  }, [value]);
  if (!draft) return <section className="page-card"><div className="empty-state"><LoaderCircle size={25} className="spin"/><strong>Loading settings…</strong></div></section>;
  const name = draft.device_name.trim();
  const nameValid = name.length > 0 && [...name].length <= 80 && ![...name].some((character) => /[\u0000-\u001F\u007F]/.test(character));
  const dirty = JSON.stringify(draft) !== JSON.stringify(value);
  const save = async () => { setSaving(true); try { await onSave({ ...draft, device_name: name }); } finally { setSaving(false); } };
  return <section className="page-card settings-card">
    <div className="settings-group"><div className="settings-group-title"><strong>This device</strong><span>How other people see you on the network</span></div><label className="field">Device name<div className="input-with-count"><input value={draft.device_name} maxLength={80} onChange={(event) => setDraft({ ...draft, device_name: event.target.value })} aria-invalid={!nameValid}/><span>{[...draft.device_name].length}/80</span></div>{!nameValid && <small className="error-text">Enter a name between 1 and 80 characters without line breaks.</small>}<small>Name changes are used for new device connections.</small></label></div>
    <div className="settings-group"><div className="settings-group-title"><strong>Received files</strong><span>Where approved downloads are saved</span></div><label className="field">Save location<div className="path-field"><input value={draft.download_location} readOnly title={draft.download_location}/><button type="button" className="secondary-button" onClick={onChooseFolder}>Change…</button></div><small>Files are written only after size and SHA-256 verification.</small></label><button type="button" className="text-button" onClick={onOpenFolder}><FolderOpen size={15}/> Open received files folder</button></div>
    <div className="settings-group"><div className="settings-group-title"><strong>Privacy and preferences</strong><span>Local-only controls</span></div><Toggle checked={draft.ask_before_receive} onChange={(checked) => setDraft({ ...draft, ask_before_receive: checked })} title="Ask before receiving files" description="Review every incoming transfer before anything is saved."/>{!draft.ask_before_receive && <div className="inline-warning"><AlertCircle size={16}/> Trusted devices will be able to save files without asking each time.</div>}<Toggle checked={draft.notifications} onChange={(checked) => setDraft({ ...draft, notifications: checked })} title="Transfer notifications" description={notificationPermission === 'granted' ? 'Desktop notifications are allowed.' : 'Your computer may ask for notification permission when you save.'}/><Toggle checked={draft.check_for_updates} onChange={(checked) => setDraft({ ...draft, check_for_updates: checked })} title="Automatically check for updates" description="Internet is used only to check signed GitHub releases."/></div>
    <div className="settings-group compact"><div className="settings-row"><div><strong>Software updates</strong><span>{updateState === 'available' ? `Version ${updateVersion} is available.` : updateState === 'current' ? 'LocalShare is up to date.' : updateState === 'offline' ? 'Could not check; LAN sharing is unaffected.' : updateState === 'checking' ? 'Checking for updates…' : updateState === 'installing' ? 'Installing update…' : 'Updates are optional and signed.'}</span></div><button type="button" className="secondary-button" onClick={onCheckUpdates} disabled={updateState === 'checking' || updateState === 'installing'}>{updateState === 'checking' ? <LoaderCircle size={15} className="spin"/> : <RefreshCw size={15}/>} Check now</button></div><div className="settings-row"><div><strong>Approved devices</strong><span>{knownPeers.length ? `${knownPeers.length} device${knownPeers.length === 1 ? '' : 's'} remembered on this computer.` : 'No devices approved yet.'}</span></div><ShieldCheck size={19} className="settings-status-icon"/></div></div>
    <div className="save-bar"><span>{dirty ? 'You have unsaved changes.' : 'All changes are saved.'}</span><button type="button" className="primary-button" onClick={() => void save()} disabled={!dirty || !nameValid || saving}>{saving ? <LoaderCircle size={16} className="spin"/> : <Check size={16}/>} Save changes</button></div>
  </section>;
}

function Toggle({ checked, onChange, title, description }: { checked: boolean; onChange: (checked: boolean) => void; title: string; description: string }) {
  return <label className="toggle-row"><span><strong>{title}</strong><small>{description}</small></span><span className="switch"><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)}/><i/></span></label>;
}

function Diagnostics({ info, peers, knownPeers, onCopy, onOpenFolder }: { info: LocalInfo | null; peers: Peer[]; knownPeers: KnownPeer[]; onCopy: () => void; onOpenFolder: () => void }) {
  const ipv4 = info?.addresses?.find((address) => !address.includes(':')) ?? 'Not detected';
  const ipv6 = info?.addresses?.find((address) => address.includes(':')) ?? 'Not detected';
  const services = [['LAN listener', info?.listener_state ?? 'starting'], ['Device discovery', info?.mdns_browse ?? 'starting'], ['Device advertising', info?.mdns_advertise ?? 'starting'], ['Encrypted transport', info?.tls_state ?? 'starting'], ['Local database', info?.database_state ?? 'starting']];
  const healthy = services.filter(([, value]) => ['active', 'healthy', 'configured'].includes(value)).length;
  const connected = peers.filter((peer) => peer.state === 'connected').length;
  return <section className="page-card diagnostics-card"><div className="section-heading page-heading"><div><h2>Connection health</h2><span className="muted">Safe to share with support; private keys are excluded</span></div><button type="button" className="secondary-button" onClick={onCopy}><Copy size={15}/> Copy diagnostics</button></div><div className="health-summary"><div className={`health-icon ${healthy >= 4 ? 'healthy' : ''}`}>{healthy >= 4 ? <CheckCircle2 size={25}/> : <Activity size={25}/>}</div><div><strong>{healthy >= 4 ? 'LocalShare is ready' : 'LocalShare is starting'}</strong><span>{healthy} of {services.length} local services ready · {connected} connected device{connected === 1 ? '' : 's'}</span></div></div><div className="diagnostics-layout"><div className="service-list">{services.map(([label, value]) => <div className="service-row" key={label}><span className={`service-dot ${['active', 'healthy', 'configured'].includes(value) ? 'healthy' : value === 'failed' ? 'failed' : ''}`}/><div><strong>{label}</strong><span>{value}</span></div></div>)}</div><div className="diagnostics-grid"><Diag label="Application" value={`LocalShare ${info?.version ?? '—'}`}/><Diag label="Device" value={`${info?.device_name ?? '—'} · ${info?.platform ?? '—'} ${info?.architecture ?? ''}`}/><Diag label="IPv4 address" value={ipv4}/><Diag label="IPv6 address" value={ipv6}/><Diag label="Listening port" value={String(info?.port ?? '—')}/><Diag label="Approved devices" value={String(knownPeers.length)}/><Diag label="Received files" value={info?.receive_directory ?? '—'} wide/><Diag label="Identity fingerprint" value={info?.fingerprint ? groupFingerprint(info.fingerprint) : '—'} wide mono/></div></div><div className="diagnostic-actions"><button type="button" className="text-button" onClick={onOpenFolder}><FolderOpen size={15}/> Open received files folder</button><span>If another device cannot find this one, allow LocalShare through your firewall and confirm both devices are on the same network.</span></div></section>;
}

function Diag({ label, value, wide = false, mono = false }: { label: string; value: string; wide?: boolean; mono?: boolean }) {
  return <div className={`diag-row ${wide ? 'wide' : ''}`}><span>{label}</span><strong className={mono ? 'mono' : ''} title={value}>{value}</strong></div>;
}

class AppErrorBoundary extends React.Component<React.PropsWithChildren, { failed: boolean; error: Error | null }> {
  state: { failed: boolean; error: Error | null } = { failed: false, error: null };
  static getDerivedStateFromError(error: Error) { return { failed: true, error }; }
  componentDidCatch(error: unknown) { console.error('LocalShare UI error', error); }
  render() {
    if (!this.state.failed) return this.props.children;
    return <main className="fatal-error">
      <AlertCircle size={34}/>
      <h1>LocalShare could not show this page</h1>
      <p>Your files are safe. Reopen the app and try again.</p>
      {this.state.error && <pre className="error-detail" style={{ fontSize: '11px', maxHeight: '100px', overflow: 'auto', background: 'rgba(0,0,0,0.06)', padding: '8px', borderRadius: '6px', maxWidth: '90vw', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{String(this.state.error?.message || this.state.error)}</pre>}
      <button type="button" className="primary-button" onClick={() => { this.setState({ failed: false, error: null }); window.location.reload(); }}>Reload LocalShare</button>
    </main>;
  }
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><AppErrorBoundary><App/></AppErrorBoundary></React.StrictMode>);
