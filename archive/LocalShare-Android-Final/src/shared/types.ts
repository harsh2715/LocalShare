export type PeerState = 'DISCOVERED' | 'UNTRUSTED' | 'PAIRING' | 'TRUSTED' | 'CONNECTING' | 'CONNECTED' | 'OFFLINE' | 'BLOCKED';
export type TransferState = 'QUEUED' | 'OFFERED' | 'ACTIVE' | 'PAUSED_NETWORK' | 'PAUSED_USER' | 'PAUSED_STORAGE_FULL' | 'PAUSED_DESTINATION_UNAVAILABLE' | 'COMPLETED' | 'FAILED' | 'CANCELLED';
export type MessageState = 'PENDING' | 'QUEUED' | 'SENT' | 'DELIVERED' | 'FAILED';
export interface Peer { id: string; name: string; platform: string; host: string; port: number; state: PeerState; fingerprint?: string; lastSeen?: number; capabilities: string[]; }
export interface ChatMessage { id: string; peerId: string; direction: 'in' | 'out'; body: string; timestamp: number; state: MessageState; }
export interface Transfer { id: string; peerId: string; direction: 'send' | 'receive'; name: string; relativePath?: string; size: number; bytesTransferred: number; state: TransferState; speed: number; etaSeconds?: number; verified: boolean; expectedHash?: string; error?: string; }
export interface Diagnostics { interfaces: Array<{ name: string; address: string; family: string; internal: boolean }>; listenerPort: number; mdns: 'ready' | 'disabled' | 'error'; peerCount: number; protocolVersion: number; internet: 'online' | 'offline' | 'unknown'; lastError?: string; }
export interface Settings { deviceName: string; discoverable: boolean; downloadFolder: string; askBeforeReceiving: boolean; networkInterface: string; transferConcurrency: number; }
export interface LocalShareState { self: { id: string; name: string; fingerprint: string; platform: string }; peers: Peer[]; transfers: Transfer[]; settings: Settings; diagnostics: Diagnostics; }
export interface LocalShareAPI {
  getState(): Promise<LocalShareState>;
  onState(listener: (state: LocalShareState) => void): () => void;
  pair(peerId: string): Promise<void>;
  forgetPeer(peerId: string): Promise<void>;
  blockPeer(peerId: string, blocked: boolean): Promise<void>;
  sendMessage(peerId: string, body: string): Promise<ChatMessage>;
  getMessages(peerId: string, before?: number): Promise<ChatMessage[]>;
  chooseFiles(): Promise<string[]>;
  sendFiles(peerId: string, paths: string[]): Promise<string[]>;
  acceptTransfer(transferId: string, directory?: string): Promise<void>;
  declineTransfer(transferId: string): Promise<void>;
  pauseTransfer(transferId: string): Promise<void>;
  resumeTransfer(transferId: string): Promise<void>;
  cancelTransfer(transferId: string): Promise<void>;
  updateSettings(settings: Partial<Settings>): Promise<Settings>;
  connectManual(host: string, port?: number, fingerprint?: string): Promise<void>;
}
