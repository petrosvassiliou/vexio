import './styles/m3-expressive.css';
import { VexioFirebaseSync, type CloudDocMeta, type CloudSyncState } from './firebase/firebaseClient';
import {
  bindWindowControlsOverlay,
  copyDiagramVectorToClipboard,
  enumerateChromeOsSystemFonts,
  loadChromeOsFontBytes,
  openLocalDiagramFile,
  pickLocalBinaryFile,
  registerChromeOsFileLaunchHandler,
  saveLocalDiagramInPlace,
} from './platform/chromeosIntegration';
import { VexioCollabSession, type PeerPresence } from './platform/collabAndVersions';
import {
  detectDefaultLanguage,
  detectDefaultMetricUnits,
  normalizeLangCode,
  PAGE_PRESET_OPTIONS,
  VEXIO_TRANSLATIONS,
  type VexioLangCode,
  type VexioStrings,
} from './platform/localization';
import type { EditableVertexDto, EngineSnapshot, SmartGuideLine } from './workers/engine.worker';
import type { OpfsVersionMeta, OpfsWorkspaceMeta } from './workers/opfs-journal.worker';

type ActiveTool =
  | 'pointer'
  | 'pan'
  | 'line'
  | 'arc'
  | 'connector'
  | 'rect'
  | 'round_rect'
  | 'ellipse'
  | 'diamond'
  | 'triangle'
  | 'hexagon'
  | 'star'
  | 'freeform'
  | 'text'
  | 'pen'
  | 'highlighter';
type InspectorTab = 'props' | 'layers' | 'comments' | 'validate';
type ActiveModal =
  | null
  | 'backstage'
  | 'updates'
  | 'vba'
  | 'sql'
  | 'data'
  | 'org'
  | 'dataviz'
  | 'cloud'
  | 'compare'
  | 'link';
type ResizeHandleDir = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

interface StarterTemplateCard {
  id: string;
  title: string;
  category: string;
  desc: string;
}

const STARTER_TEMPLATES: StarterTemplateCard[] = [
  { id: 'bpmn_fulfillment', title: 'BPMN 2.0 Order Fulfillment', category: 'Business Process', desc: 'Gateways, tasks & sequence flows ready for BPMN validation' },
  { id: 'cloud_microservices', title: 'Cloud Kubernetes Architecture', category: 'Cloud (AWS/GCP/Azure)', desc: 'Edge CDN, K8s cluster, serverless & multi-zone VPC container' },
  { id: 'sql_ecommerce_erd', title: "E-Commerce Crow's Foot ERD", category: 'Database Schema', desc: '4 relational tables with PK/FK Crow’s Foot connectors' },
  { id: 'executive_org_chart', title: 'Executive Organization Chart', category: 'Org Chart', desc: 'Role-styled reporting hierarchy built from roster CSV' },
  { id: 'release_gantt', title: 'Product Launch Gantt Schedule', category: 'Project Schedule', desc: 'Proportional task bars, completion % fills & FS dependencies' },
  { id: 'project_timeline', title: 'Annual Architecture Roadmap', category: 'Timeline', desc: 'Executive spine with major/minor diamond milestones' },
  { id: 'datacenter_floorplan', title: 'Data Center Floor Plan & Rack', category: 'Facilities & Network', desc: 'Architectural scaled rooms + 42U spine rack' },
  { id: 'blank', title: 'Blank Visio Drawing', category: 'ISO Metric / Standard', desc: 'Clean ISO A4 (297×210 mm) or US Letter sheet with default layers' },
];

const FALLBACK_STENCILS: Record<string, string[]> = {
  'Basic Shapes': [
    'Rectangle',
    'Rounded Rectangle',
    'Snip-Corner Rectangle',
    'Beveled Frame',
    'Square',
    'Circle',
    'Ellipse',
    'Semi-Circle',
    'Quarter Pie Sector',
    'Triangle',
    'Right Triangle',
    'Inverted Triangle',
    'Diamond',
    'Parallelogram',
    'Trapezoid',
    'Pentagon',
    'Hexagon',
    'Heptagon',
    'Octagon',
    'Decagon',
    'Star (4-Point)',
    'Star (5-Point)',
    'Star (6-Point)',
    'Star (8-Point)',
    'Cross / Plus',
    'Cylinder / Can',
    '3D Isometric Cube',
    'Ring / Frame',
  ],
  'Basic Flowchart': [
    'Process',
    'Decision',
    'Start/End',
    'Data (I/O)',
    'Subprocess',
    'Document',
    'Multi-Document',
    'Database',
    'Manual Input',
    'Manual Operation',
    'Preparation',
    'Delay',
    'On-Page Reference',
    'Off-Page Reference',
    'Internal Storage',
    'Sequential Data (Tape)',
    'Direct Data (Drum)',
    'Stored Data',
    'Punched Card',
    'Paper Tape',
    'Display',
    'Merge',
    'Or Junction',
    'Summing Junction',
  ],
  'Microsoft 365 & Office': [
    'Microsoft 365 Tenant',
    'Microsoft Teams',
    'Exchange Online',
    'SharePoint Online',
    'OneDrive for Business',
    'Outlook Mail Flow',
    'Word Document',
    'Excel Workbook',
    'PowerPoint Deck',
    'OneNote Notebook',
    'Microsoft Visio',
    'Microsoft Project',
    'Microsoft Access DB',
    'Microsoft Publisher',
    'Microsoft Copilot',
    'Microsoft Loop',
    'Microsoft Forms',
    'Microsoft Planner',
    'Microsoft To Do',
    'Microsoft Stream',
    'Microsoft Whiteboard',
    'Viva Engage',
    'Viva Insights',
    'Microsoft Intune',
    'Defender for M365',
    'Microsoft Purview',
  ],
  'Microsoft Servers & Active Directory': [
    'Windows Server',
    'Windows Server Core',
    'Active Directory DC',
    'Read-Only DC (RODC)',
    'Microsoft Entra ID',
    'Entra Connect Sync',
    'AD Forest / Domain Trust',
    'Organizational Unit (OU)',
    'Group Policy (GPO)',
    'AD Certificate Services (PKI)',
    'AD Federation Services (ADFS)',
    'DNS / DHCP Server',
    'Windows DNS Server',
    'Windows DHCP Server',
    'IIS Web Server',
    'SQL Server Cluster',
    'SQL Reporting (SSRS)',
    'SQL Integration (SSIS)',
    'Exchange Server',
    'SharePoint Server',
    'Hyper-V Host',
    'Failover Cluster',
    'System Center (MECM / SCOM)',
    'WSUS Patch Server',
    'Windows 11 Workstation',
  ],
  'Microsoft Azure & Hybrid Cloud': [
    'Azure Subscription / RG',
    'Management Group',
    'Azure Virtual Machine',
    'VM Scale Set (VMSS)',
    'Azure Kubernetes (AKS)',
    'Azure Container Apps',
    'Azure App Service',
    'Azure Functions',
    'Azure Logic Apps',
    'API Management (APIM)',
    'Azure SQL / Cosmos DB',
    'Azure SQL Database',
    'Azure Storage Account',
    'Azure Data Lake Gen2',
    'Azure Virtual Network (VNet)',
    'Network Security Group (NSG)',
    'Azure Firewall',
    'Azure Front Door / App GW',
    'Application Gateway (WAF)',
    'Azure Load Balancer',
    'Azure Bastion / NAT GW',
    'Azure VPN Gateway',
    'Azure Key Vault',
    'Azure Service Bus / Event Hub',
    'Azure Event Hub',
    'Azure Monitor / Log Analytics',
    'Azure ExpressRoute / VPN',
    'Azure Arc (Hybrid)',
    'Azure AI Foundry / OpenAI',
  ],
  'Microsoft Power Platform & Dynamics 365': [
    'Power BI Dashboard',
    'Power BI Semantic Model',
    'Power BI Paginated Report',
    'Power Automate Flow',
    'Power Automate Desktop (RPA)',
    'Power Apps',
    'Power Apps Model-Driven',
    'Microsoft Dataverse',
    'AI Builder Model',
    'Copilot Studio Agent',
    'Power Pages Portal',
    'Custom API Connector',
    'On-Premises Data Gateway',
    'Managed Environment',
    'Dynamics 365 Sales / CRM',
    'Dynamics 365 Customer Service',
    'Dynamics 365 Field Service',
    'Dynamics 365 Finance & ERP',
    'Dynamics 365 Business Central',
    'Microsoft Fabric Lakehouse',
  ],
  'Servers (Standard Roles)': [
    'Tower Server',
    'Rackmount Server (1U / 2U)',
    'Rackmount Server (4U)',
    'Blade Server Chassis',
    'Blade Server Module',
    'Mainframe Host',
    'Microserver / Edge Node',
    'Web Server',
    'Database Server',
    'Application Server',
    'File Server',
    'Print Server',
    'Mail / SMTP Server',
    'Directory / LDAP Server',
    'DNS Server',
    'DHCP Server',
    'NTP Time Server',
    'Proxy / Caching Server',
    'Firewall / Bastion Server',
    'FTP / SFTP Transfer Server',
    'Syslog / SIEM Server',
    'Monitoring / NMS Server',
    'PKI Certificate Server',
    'Terminal / RDS Server',
    'Virtualization / Hypervisor Host',
    'Backup & Storage Server',
  ],
  'Network & Rack Equipment': [
    'Enterprise Firewall',
    'Next-Gen Firewall (NGFW)',
    'Web Application Firewall (WAF)',
    'IDS / IPS Security Appliance',
    'DDoS Mitigation Appliance',
    'VPN Concentrator / Gateway',
    'NAC / RADIUS Appliance',
    'Core Switch (1U)',
    'Layer 2 Access Switch',
    'Layer 3 Multilayer Switch',
    'Chassis Core Switch',
    'PoE+ Managed Switch',
    'Fiber / SAN Switch',
    'Industrial DIN Switch',
    'Core Router',
    'Edge / Border Router (BGP)',
    'Branch ISR Router',
    'SD-WAN Edge Gateway',
    'Load Balancer',
    'Reverse Proxy / API Gateway',
    'Wi-Fi Access Point',
    'Outdoor Mesh AP',
    'Wireless LAN Controller (WLC)',
    'Patch Panel (48-Port)',
    'Optical ODF Splice Tray',
    'Cable / Fiber Modem (ONT)',
    'Media Converter (Fiber/RJ45)',
    'NAS / SAN Disk Array',
    'Tape Library Autoloader',
    'UPS Battery Backup',
    'Rack PDU Power Strip',
    '42U Standard Rack',
    'Cloud VPC',
    'Database Cluster',
    'Workstation / Laptop',
    'VoIP Desk Phone',
    'IP Security Camera',
    'Network Laser Printer',
  ],
  'Arrows & Block Arrows': [
    'Right Block Arrow',
    'Left Block Arrow',
    'Up Block Arrow',
    'Down Block Arrow',
    'Bidirectional Arrow',
    'Quad 4-Way Arrow',
    'Chevron',
    'Bent 90° Arrow',
    'U-Turn Arrow',
    'Notched Signal Arrow',
  ],
  'Callouts, Notes & Containers': [
    'Callout Bubble',
    'Folded Note',
    'Section Container',
    'Curly Brace Left',
    'Curly Brace Right',
    'Banner Ribbon',
    'Status Badge',
    'Title Header',
  ],
  'Cross-Functional Swimlanes': [
    'Horizontal Swimlane',
    'Vertical Swimlane',
    'Pool Header Frame',
    'Phase Divider',
    'Handoff Node',
    'Approval Gate',
  ],
  'BPMN 2.0': [
    'Start Event',
    'Message Start Event',
    'Timer Event',
    'Intermediate Event',
    'End Event',
    'Task',
    'User Task',
    'Service Task',
    'Collapsed Subprocess',
    'Exclusive Gateway',
    'Parallel Gateway',
    'Inclusive Gateway',
    'Data Object',
    'Participant Pool',
  ],
  'UML 2.5': [
    'Class',
    'Interface',
    'Component',
    'Package',
    'Use Case',
    'State',
    'Activity Initial',
    'Activity Final',
    'Actor / Lifeline',
    'Deployment Node (3D)',
    'UML Note',
  ],
  'Cloud Architecture (AWS / GCP / Azure)': [
    'Compute VM',
    'Kubernetes Cluster',
    'Serverless Function',
    'Object Storage Bucket',
    'Managed SQL / Spanner',
    'Edge CDN / Load Balancer',
    'Pub/Sub Event Bus',
    'API Gateway',
    'IAM Security Shield',
  ],
  "Database Crow's Foot & IDEF1X": [
    'Strong Entity',
    'Weak / Associative Entity',
    'Materialized View',
    'PK Attribute',
    'FK Attribute',
    'Index / Trigger',
  ],
  'Floor Plan & Architecture': [
    'Exterior Wall',
    'Interior Partition',
    'Single Door (90°)',
    'Double French Door',
    'Casement Window',
    'Staircase Flight',
    'Conference Table',
    'Office Desk Workstation',
    'Elevator Shaft',
  ],
  'P&ID Process Engineering': [
    'Centrifugal Pump',
    'Control Valve',
    'Check Valve',
    'Heat Exchanger',
    'Vertical Storage Tank',
    'Pressure Vessel',
    'Inline Flow Meter',
  ],
  'Electrical & Electronic (IEEE / IEC)': [
    'Resistor (IEEE)',
    'Capacitor',
    'Inductor',
    'Step-Down Transformer',
    'PN Diode',
    'NPN Transistor',
    'Op-Amp',
    'Single-Pole Switch',
    'DC Battery',
    'Earth Ground',
    'AC Voltage Source',
  ],
  'Organization Chart': [
    'Executive',
    'VP / Director',
    'Manager',
    'Individual Contributor',
    'Consultant / Contractor',
    'Executive Assistant',
    'Open Vacancy',
  ],
  'UI & Application Wireframes': [
    'Browser / Desktop Frame',
    'Mobile Phone Frame',
    'Top App Bar',
    'Navigation Rail',
    'KPI Metric Card',
    'Data Grid Table',
    'Primary Action Button',
    'Search Input',
    'Modal Dialog',
  ],
  'Project Schedule (Gantt & Timeline)': [
    'Timeline Spine',
    'Milestone Diamond',
    'Phase Interval Bar',
    'Gantt Task Bar',
    'Summary Rollup Bar',
    'Checkpoint Flag',
  ],
  'Value Stream Mapping (Lean VSM)': [
    'Customer / Supplier',
    'Dedicated Process',
    'Shared Process',
    'Inventory Triangle',
    'Supermarket',
    'Kaizen Burst',
    'FIFO Lane',
    'Truck Shipment',
  ],
};

const DEFAULT_VBA_SCRIPT = `Sub BuildCluster()
  Begin UserForm DeployForm
    Caption = "Cluster Provisioning"
    Control TextBox txtEnv = "Prod"
  End
  For i = 1 To 3
    Set node = ActivePage.DrawRectangle(1.2 + i * 2.2, 2.2, 2.7 + i * 2.2, 3.1)
    If i = 2 Then
      node.Text = "Gateway (" & DeployForm.txtEnv & ")"
    Else
      node.Text = "Microservice " & i
    End If
    node.Cells("FillForegnd") = "RGB(219,234,254)"
  Next i
End Sub`;

const DEFAULT_SQL_DDL = `CREATE TABLE organizations (
  id INT PRIMARY KEY,
  name VARCHAR NOT NULL,
  plan_tier VARCHAR NOT NULL
);

CREATE TABLE workspaces (
  id INT PRIMARY KEY,
  org_id INT NOT NULL REFERENCES organizations(id),
  title VARCHAR NOT NULL,
  region VARCHAR
);

CREATE TABLE diagrams (
  id INT PRIMARY KEY,
  workspace_id INT NOT NULL REFERENCES workspaces(id),
  format VARCHAR NOT NULL
);`;

const DEFAULT_TELEMETRY_CSV = `Label,Status,Load
Validate Order & Inventory,Healthy,88
In Stock?,Warning,52`;

const DEFAULT_ORG_CSV = `Name,Title,Manager,Department
Elena Rostova,Chief Executive Officer,,Executive
Marcus Vance,VP Systems Engineering,Elena Rostova,Engineering
Priya Nair,VP Product & Design,Elena Rostova,Product
Liam O'Connor,Rust Graphics Lead,Marcus Vance,Core Engine
Sofia Lindqvist,Platform UI Lead,Priya Nair,Design Systems`;

const DEFAULT_DATAVIZ_CSV = `Process Step ID,Process Step Description,Next Step ID,Connector Label,Shape Type,Function,Owner
S1,Receive Customer Request,S2,,Start,Customer Portal,Support
S2,Automated Fraud Check,S3;S4,Pass;Review,Decision,Risk Engine,SecOps
S3,Provision Subscription,S5,,Process,Billing Core,Finance
S4,Manual Compliance Audit,S5,Approved,Subprocess,Risk Engine,Compliance
S5,Order Completed,,,End,Customer Portal,Support`;

const DEFAULT_CLOUD_JSON = JSON.stringify(
  {
    provider: 'aws',
    vpcs: [{ id: 'vpc-prod', name: 'Production Multi-AZ VPC', cidr: '10.20.0.0/16' }],
    resources: [
      { id: 'cf-edge', name: 'CloudFront Edge', kind: 'cdn', vpc: 'vpc-prod' },
      { id: 'eks-api', name: 'EKS Rust Core', kind: 'compute', vpc: 'vpc-prod' },
      { id: 'aurora-pg', name: 'Aurora PostgreSQL', kind: 'database', vpc: 'vpc-prod' },
    ],
    edges: [
      { from: 'cf-edge', to: 'eks-api', label: 'HTTPS/3' },
      { from: 'eks-api', to: 'aurora-pg', label: 'TLS 1.3' },
    ],
  },
  null,
  2
);

interface StencilPreviewItem {
  name: string;
  prompt: string;
  svg: string;
}

interface DiagramDiffReportDto {
  added_count: number;
  removed_count: number;
  modified_count: number;
  entries: Array<{ page_name: string; shape_id: number; shape_name: string; kind: unknown; summary: string }>;
  diff_overlay_svg: string;
}

const UI_ICONS = {
  pointer: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><path d="M3.5 2.2 L12.5 8.8 L8.1 9.5 L6.2 13.8 Z"/></svg>',
  pan: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M6 8V3.2a1 1 0 0 1 2 0V7m0-3.2a1 1 0 0 1 2 0V7m0-2.4a1 1 0 0 1 2 0V9.5a4 4 0 0 1-4 4H6.8a4 4 0 0 1-3.3-1.8L2.2 9.6a1 1 0 0 1 1.6-1.2L6 9.8Z"/></svg>',
  line: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><line x1="2.5" y1="13.5" x2="13.5" y2="2.5"/><circle cx="2.5" cy="13.5" r="1.4" fill="currentColor"/><circle cx="13.5" cy="2.5" r="1.4" fill="currentColor"/></svg>',
  arc: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M2.5 12.5 A 9 9 0 0 1 13.5 3.5"/><circle cx="2.5" cy="12.5" r="1.3" fill="currentColor"/><circle cx="13.5" cy="3.5" r="1.3" fill="currentColor"/></svg>',
  connector: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="3.5" cy="12.5" r="1.8"/><circle cx="12.5" cy="3.5" r="1.8"/><path d="M5.3 12.5 H8.5 V3.5 H10.7"/></svg>',
  rect: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2.2" y="3.2" width="11.6" height="9.6" rx="0.8"/></svg>',
  round_rect: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2.2" y="3.2" width="11.6" height="9.6" rx="3.4"/></svg>',
  ellipse: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><ellipse cx="8" cy="8" rx="6" ry="4.8"/></svg>',
  diamond: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><polygon points="8,2 14.2,8 8,14 1.8,8"/></svg>',
  triangle: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><polygon points="8,2.2 14.2,13.5 1.8,13.5"/></svg>',
  hexagon: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><polygon points="4.8,2.5 11.2,2.5 14.2,8 11.2,13.5 4.8,13.5 1.8,8"/></svg>',
  star: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><polygon points="8,1.8 9.8,5.8 14.2,6.2 10.8,9.1 11.9,13.5 8,11.1 4.1,13.5 5.2,9.1 1.8,6.2 6.2,5.8"/></svg>',
  freeform: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M2 12 C4.5 3, 9.5 13, 14 4"/></svg>',
  text: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M3 3.2 H13 M8 3.2 V13 M5.5 13 H10.5"/></svg>',
  pen: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M11.2 2.2 L13.8 4.8 L5.5 13.1 L2.2 13.8 L2.9 10.5 Z"/></svg>',
  highlighter: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9.8 2.4 L13.6 6.2 L6.8 13 L3 9.2 Z"/><path d="M2 14 H7"/></svg>',
  duplicate: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><rect x="5.2" y="5.2" width="8.5" height="8.5" rx="1.4"/><path d="M10.5 5.2 V3.3 A1 1 0 0 0 9.5 2.3 H3.3 A1 1 0 0 0 2.3 3.3 V9.5 A1 1 0 0 0 3.3 10.5 H5.2"/></svg>',
  trash: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M3 4.5 H13 M6 4.5 V2.8 H10 V4.5 M4.5 4.5 L5.2 13.5 H10.8 L11.5 4.5"/></svg>',
  open: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><path d="M2 4.2 H6.2 L7.6 5.8 H14 V12.8 H2 Z"/></svg>',
  save: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><path d="M2.5 2.5 H11.2 L13.5 4.8 V13.5 H2.5 Z"/><path d="M5 2.5 V6 H10.5 V2.5 M4.8 13.5 V9.2 H11.2 V13.5"/></svg>',
  undo: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M5.5 4 L2.2 7.2 L5.5 10.5"/><path d="M2.5 7.2 H9.8 C12.1 7.2 13.8 8.8 13.8 11.2"/></svg>',
  redo: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10.5 4 L13.8 7.2 L10.5 10.5"/><path d="M13.5 7.2 H6.2 C3.9 7.2 2.2 8.8 2.2 11.2"/></svg>',
  snapshot: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8" cy="8" r="5.5"/><path d="M8 5.2 V8.2 L10.2 9.6"/></svg>',
  sun: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8" cy="8" r="3"/><path d="M8 1.5 V3 M8 13 V14.5 M1.5 8 H3 M13 8 H14.5 M3.4 3.4 L4.5 4.5 M11.5 11.5 L12.6 12.6 M12.6 3.4 L11.5 4.5 M4.5 11.5 L3.4 12.6"/></svg>',
  moon: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><path d="M13.2 9.8 A5.5 5.5 0 0 1 6.2 2.8 A5.5 5.5 0 1 0 13.2 9.8 Z"/></svg>',
  sidebarLeft: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"><rect x="2" y="2.5" width="12" height="11" rx="1.8"/><path d="M6.2 2.5 V13.5"/></svg>',
  sidebarRight: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"><rect x="2" y="2.5" width="12" height="11" rx="1.8"/><path d="M9.8 2.5 V13.5"/></svg>',
  search: '<svg class="search-box-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5 L13.8 13.8"/></svg>',
};

function buildFallbackStencilSvg(masterName: string): string {
  const low = masterName.toLowerCase();
  if (low.includes('ring') || low.includes('frame') && low.includes('ring')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><circle cx="72" cy="48" r="32" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/><circle cx="72" cy="48" r="20" fill="#f8fafc" stroke="#2563eb" stroke-width="2"/></svg>';
  }
  if (low.includes('ellipse') || low.includes('circle') || low.includes('router') || low.includes('pump') || low.includes('use case')) {
    const rx = low.includes('circle') || low.includes('router') || low.includes('pump') ? 29 : 44;
    return `<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><ellipse cx="72" cy="48" rx="${rx}" ry="29" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>`;
  }
  if (low.includes('triangle') || low.includes('op-amp') || low.includes('ground') || low.includes('inventory')) {
    const pts = low.includes('right') ? '30,18 116,78 30,78' : '72,16 118,78 26,78';
    return `<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="${pts}" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>`;
  }
  if (low.includes('trapezoid')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="36,22 108,22 124,74 20,74" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>';
  }
  if (low.includes('pentagon') || low.includes('shield')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="72,14 116,40 100,80 44,80 28,40" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>';
  }
  if (low.includes('octagon') || low.includes('api gateway')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="52,16 92,16 118,36 118,60 92,80 52,80 26,60 26,36" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>';
  }
  if (low.includes('hexagon') || low.includes('preparation') || low.includes('kubernetes') || low.includes('serverless')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="44,18 100,18 124,48 100,78 44,78 20,48" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>';
  }
  if (low.includes('cross') || low.includes('plus')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="58,16 86,16 86,36 108,36 108,60 86,60 86,80 58,80 58,60 36,60 36,36 58,36" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>';
  }
  if (low.includes('cube') || low.includes('3d')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="34,30 88,30 108,16 54,16" fill="#dbeafe" stroke="#2563eb" stroke-width="2"/><rect x="34" y="30" width="54" height="50" fill="#eff6ff" stroke="#2563eb" stroke-width="2"/><polygon points="88,30 108,16 108,66 88,80" fill="#bfdbfe" stroke="#2563eb" stroke-width="2"/></svg>';
  }
  if (low.includes('star') || low.includes('kaizen')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="72,14 82,36 106,38 88,54 94,78 72,65 50,78 56,54 38,38 62,36" fill="#fef9c3" stroke="#ca8a04" stroke-width="2.2"/></svg>';
  }
  if (low.includes('left block arrow')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="122,36 56,36 56,22 20,48 56,74 56,60 122,60" fill="#dbeafe" stroke="#1d4ed8" stroke-width="2.2"/></svg>';
  }
  if (low.includes('up block arrow')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="58,82 86,82 86,42 102,42 72,14 42,42 58,42" fill="#dbeafe" stroke="#1d4ed8" stroke-width="2.2"/></svg>';
  }
  if (low.includes('down block arrow')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="58,14 86,14 86,54 102,54 72,82 42,54 58,54" fill="#dbeafe" stroke="#1d4ed8" stroke-width="2.2"/></svg>';
  }
  if (low.includes('bidirectional')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="18,48 44,24 44,38 100,38 100,24 126,48 100,72 100,58 44,58 44,72" fill="#dbeafe" stroke="#1d4ed8" stroke-width="2.2"/></svg>';
  }
  if (low.includes('chevron') || low.includes('handoff')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="22,22 98,22 122,48 98,74 22,74 44,48" fill="#dbeafe" stroke="#1d4ed8" stroke-width="2.2"/></svg>';
  }
  if (low.includes('arrow')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="22,36 88,36 88,22 124,48 88,74 88,60 22,60" fill="#dbeafe" stroke="#1d4ed8" stroke-width="2.2"/></svg>';
  }
  if (low.includes('callout')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="22,18 122,18 122,64 64,64 40,82 46,64 22,64" fill="#fef9c3" stroke="#ca8a04" stroke-width="2.2"/></svg>';
  }
  if (low.includes('note') || low.includes('data object')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="32,14 96,14 112,30 112,82 32,82" fill="#fef9c3" stroke="#ca8a04" stroke-width="2.2"/><polyline points="96,14 96,30 112,30" fill="none" stroke="#ca8a04" stroke-width="2"/></svg>';
  }
  if (low.includes('brace')) {
    const d = low.includes('right') ? 'M54,16 Q72,16 72,30 V42 Q72,48 88,48 Q72,48 72,54 V66 Q72,80 54,80' : 'M90,16 Q72,16 72,30 V42 Q72,48 56,48 Q72,48 72,54 V66 Q72,80 90,80';
    return `<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><path d="${d}" fill="none" stroke="#475569" stroke-width="2.5" stroke-linecap="round"/></svg>`;
  }
  if (low.includes('cylinder') || low.includes('can') || low.includes('database') || low.includes('tank')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><path d="M40,26 V70 A32,10 0 0,0 104,70 V26 Z" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/><ellipse cx="72" cy="26" rx="32" ry="10" fill="#dbeafe" stroke="#2563eb" stroke-width="2.2"/></svg>';
  }
  if (low.includes('gateway') || low.includes('decision') || low.includes('milestone') || low.includes('diamond') || low.includes('approval') || low.includes('valve')) {
    const inner = low.includes('exclusive')
      ? '<path d="M62 40 L82 60 M82 40 L62 60" stroke="#ca8a04" stroke-width="2.2" stroke-linecap="round"/>'
      : low.includes('parallel')
        ? '<path d="M72 36 V64 M58 50 H86" stroke="#ca8a04" stroke-width="2.2" stroke-linecap="round"/>'
        : '';
    return `<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="72,14 122,48 72,82 22,48" fill="#fef9c3" stroke="#ca8a04" stroke-width="2.2"/>${inner}</svg>`;
  }
  if (low.includes('event')) {
    const isEnd = low.includes('end');
    const isInt = low.includes('intermediate') || low.includes('timer');
    const col = isEnd ? '#dc2626' : isInt ? '#d97706' : '#059669';
    const bg = isEnd ? '#fef2f2' : isInt ? '#fffbeb' : '#ecfdf5';
    return `<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><circle cx="72" cy="48" r="28" fill="${bg}" stroke="${col}" stroke-width="${isEnd ? 3.6 : 2.2}"/>${isInt ? `<circle cx="72" cy="48" r="22" fill="none" stroke="${col}" stroke-width="1.8"/>` : ''}</svg>`;
  }
  if (low.includes('start') || low.includes('terminator') || low.includes('badge') || low.includes('button')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><rect x="20" y="26" width="104" height="44" rx="22" fill="#ecfdf5" stroke="#0d9488" stroke-width="2.2"/></svg>';
  }
  if (low.includes('data') || low.includes('parallelogram')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><polygon points="34,22 126,22 110,74 18,74" fill="#f0f9ff" stroke="#0284c7" stroke-width="2.2"/></svg>';
  }
  if (low.includes('document')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><path d="M24,20 H120 V68 Q96,82 72,68 Q48,54 24,68 Z" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>';
  }
  if (low.includes('swimlane') || low.includes('pool') || low.includes('container')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><rect x="18" y="20" width="108" height="56" rx="3" fill="#f8fafc" stroke="#475569" stroke-width="2"/><line x1="40" y1="20" x2="40" y2="76" stroke="#475569" stroke-width="2"/></svg>';
  }
  if (low.includes('subprocess')) {
    return '<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><rect x="20" y="20" width="104" height="56" rx="4" fill="#eef2ff" stroke="#4f46e5" stroke-width="2.2"/><path d="M34 20 V76 M110 20 V76" stroke="#4f46e5" stroke-width="1.8"/></svg>';
  }
  const rx = low.includes('rounded') ? 16 : 3;
  const w = low.includes('square') ? 60 : 100;
  const x = (144 - w) / 2;
  return `<svg viewBox="0 0 144 96" xmlns="http://www.w3.org/2000/svg"><rect x="${x}" y="20" width="${w}" height="56" rx="${rx}" fill="#eff6ff" stroke="#2563eb" stroke-width="2.2"/></svg>`;
}

function pointToSegmentDist(px: number, py: number, x1: number, y1: number, x2: number, y2: number): number {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const lenSq = dx * dx + dy * dy;
  if (lenSq <= 1e-9) return Math.hypot(px - x1, py - y1);
  const t = Math.max(0, Math.min(1, ((px - x1) * dx + (py - y1) * dy) / lenSq));
  return Math.hypot(px - (x1 + t * dx), py - (y1 + t * dy));
}

interface ContextMenuState {
  kind: 'shape' | 'canvas' | 'page' | 'stencil';
  x: number;
  y: number;
  pageIn?: [number, number];
  targetPageId?: number;
  stencilPack?: string;
  stencilMaster?: string;
}

class VexioChromeOsApp {
  private engineWorker: Worker;
  private opfsWorker: Worker;
  private firebaseSync = new VexioFirebaseSync();
  private collabSession!: VexioCollabSession;
  private rpcSeq = 1;
  private pendingRpc = new Map<number, { resolve: (v: any) => void; reject: (e: any) => void }>();
  private pendingOpfs = new Map<number, { resolve: (v: any) => void; reject: (e: any) => void }>();

  // Persistent WebGPU OffscreenCanvas element preserved across DOM re-renders
  private wgpuCanvasEl: HTMLCanvasElement;
  private offscreenTransferred = false;
  private wgpuAttached = false;

  private docId = 'workspace-default';
  private activeFileHandle: FileSystemFileHandle | null = null;
  private currentFileName = 'Architecture-Diagram.vsdx';
  private snapshot: EngineSnapshot | null = null;
  private stencilCatalog: Record<string, string[]> = FALLBACK_STENCILS;
  private stencilPreviews: Record<string, StencilPreviewItem[]> = {};
  private packOrder: string[] = Object.keys(FALLBACK_STENCILS);
  private stencilGroupFilter: 'all' | 'microsoft' | 'network' | 'basic' | 'cloud' | 'eng' = 'all';
  private expandedStencilPacks = new Set<string>([
    'Microsoft 365 & Office',
    'Servers (Standard Roles)',
    'Network & Rack Equipment',
  ]);
  private cloudState: CloudSyncState = {
    initialized: false,
    online: navigator.onLine,
    user: null,
    lastSyncedIso: null,
  };

  private peers: PeerPresence[] = [];
  private opfsVersions: OpfsVersionMeta[] = [];
  private opfsWorkspaces: OpfsWorkspaceMeta[] = [];
  private cloudWorkspaces: CloudDocMeta[] = [];
  private syncDebounceTimer: number | null = null;

  private uiTheme: 'light' | 'dark' = 'light';
  private lang: VexioLangCode = detectDefaultLanguage();
  private metricUnits: boolean = detectDefaultMetricUnits();
  private activePagePresetId: string = this.metricUnits ? 'a4_l' : 'letter_l';
  private isRibbonCollapsed = false;
  private openHeaderMenu: null | 'file' | 'edit' | 'export' | 'help' = null;
  private isRenamingDocTitle = false;
  private renameDocDraft = '';
  private renamingPageId: number | null = null;
  private renamePageDraft = '';
  private launchedWithExternalFile = false;

  private textFontFamily = 'Inter';
  private textFontSizePt = 11;
  private textBold = false;
  private textItalic = false;
  private textUnderline = false;
  private textStrikethrough = false;
  private textHorizAlign = 1;
  private textVertAlign = 1;
  private textBullets = false;
  private textColorHex = '#0F172A';
  private textBgColorHex = 'none';
  private shapeFillHex = '#EFF6FF';
  private shapeLineHex = '#1E3A8A';

  private autoCheckUpdates: boolean = (() => {
    try {
      return localStorage.getItem('vexio.autoUpdate') !== 'false';
    } catch {
      return true;
    }
  })();
  private updateStatusText = 'Vexio v0.3.0-alpha • Automatic updates enabled';
  private latestReleaseTag: string | null = null;
  private latestReleaseUrl = 'https://github.com/petrosvassiliou/vexio/releases';

  private activeRibbonTab = 'Home';
  private activeTool: ActiveTool = 'pointer';
  private spaceKeyDown = false;
  private activeInspectorTab: InspectorTab = 'props';
  // Open the in-app Start / File Backstage hub inside Vexio on startup (never forcing an external OS file picker)
  private activeModal: ActiveModal = 'backstage';
  private modalOutputMessage = '';
  private vbaDraft = DEFAULT_VBA_SCRIPT;
  private sqlDraft = DEFAULT_SQL_DDL;
  private csvDraft = DEFAULT_TELEMETRY_CSV;
  private orgCsvDraft = DEFAULT_ORG_CSV;
  private dataVizDraft = DEFAULT_DATAVIZ_CSV;
  private cloudJsonDraft = DEFAULT_CLOUD_JSON;
  private versionLabelDraft = 'Milestone Checkpoint';
  private selectedThemeSlug = 'modern';
  private selectedVariantIx = 0;
  private linkAddressDraft = 'https://docs.vexio.dev/runbook';
  private linkSubAddressDraft = '';
  private linkScreentipDraft = 'Click or hover to inspect architecture SLA & owner';
  private latestDiffReport: DiagramDiffReportDto | null = null;

  private activeStencilPack = 'Basic Shapes';
  private stencilFilter = '';
  private selectedShapeIds: number[] = [];
  private clipboardShapeIds: number[] = [];
  private contextMenu: ContextMenuState | null = null;
  private hoveredShapeId: number | null = null;
  private editingShapeId: number | null = null;
  private inlineTextDraft = '';
  private pendingInlineSelectAll = false;
  private inlineSelectAllGuardUntilMs = 0;
  private isRebuildingDom = false;
  private lastCanvasClick: { shapeId: number; timeMs: number; clientX: number; clientY: number } | null = null;
  private toastBanner: { text: string; level: 'info' | 'warn' | 'error' } | null = null;
  private toastTimer: number | null = null;

  private showLeftStencils = true;
  private showRightInspector = true;
  private showFormulaBar = false;
  private showShapeSheet = false;
  private showRulers = true;
  private showGrid = true;
  private showSmartGuides = true;
  private showMinimap = false;
  private activeFormulaCell = 'Width';
  private activeFormulaDraft = '=2.4 in';
  private showVertexGrips = false;
  private activeShapeVertices: EditableVertexDto[] = [];
  private zoomPct = 82;
  private cursorCoordsIn: [number, number] = [0, 0];
  private systemFonts: string[] = [];
  private smartGuideReqInFlight = false;
  private latestSmartGuides: SmartGuideLine[] = [];
  private latestSnappedPin: [number, number] | null = null;

  private get i18n(): VexioStrings {
    return VEXIO_TRANSLATIONS[normalizeLangCode(this.lang)];
  }

  private formatLengthFromIn(inches: number): string {
    if (this.metricUnits) {
      return `${(inches * 25.4).toFixed(1)} mm`;
    }
    return `${inches.toFixed(2)}"`;
  }

  private hexToRgbTriplet(hex: string): string {
    const clean = hex.trim().replace(/^#/, '');
    if (clean.length === 6) {
      const r = parseInt(clean.slice(0, 2), 16);
      const g = parseInt(clean.slice(2, 4), 16);
      const b = parseInt(clean.slice(4, 6), 16);
      if (!Number.isNaN(r) && !Number.isNaN(g) && !Number.isNaN(b)) {
        return `${r},${g},${b}`;
      }
    }
    return '239,246,255';
  }

  private parseFormulaOrValueHex(formula?: string, val?: string): string | null {
    for (const s of [val, formula]) {
      if (!s) continue;
      const hexMatch = s.match(/#([0-9a-fA-F]{6})\b/);
      if (hexMatch) return `#${hexMatch[1].toUpperCase()}`;
      const rgbMatch = s.match(/RGB\s*\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)/i);
      if (rgbMatch) {
        const to2 = (n: string) => Math.max(0, Math.min(255, Number(n))).toString(16).padStart(2, '0');
        return `#${to2(rgbMatch[1])}${to2(rgbMatch[2])}${to2(rgbMatch[3])}`.toUpperCase();
      }
    }
    return null;
  }

  private async applyShapeFillHex(hex: string): Promise<void> {
    this.shapeFillHex = hex.toUpperCase();
    if (this.selectedShapeIds.length === 0) {
      this.render();
      return;
    }
    const rgb = this.hexToRgbTriplet(this.shapeFillHex);
    let lastRes: any = null;
    let blocked = false;
    for (const sid of this.selectedShapeIds) {
      lastRes = await this.callEngine('setCellFormula', {
        shapeId: sid,
        cellName: 'FillForegnd',
        formula: `=RGB(${rgb})`,
        forceSetf: false,
      });
      if (lastRes?.applied === false) blocked = true;
    }
    if (blocked) {
      this.showToast('FillForegnd is protected by GUARD() on one or more selected shapes.', 'warn');
    }
    if (lastRes) {
      this.persistMutationSideEffects(lastRes.wal);
      this.snapshot = lastRes.snapshot;
      this.render();
    }
  }

  private async applyShapeLineHex(hex: string): Promise<void> {
    this.shapeLineHex = hex.toUpperCase();
    if (this.selectedShapeIds.length === 0) {
      this.render();
      return;
    }
    const rgb = this.hexToRgbTriplet(this.shapeLineHex);
    const res = await this.callEngine('batchSetCells', {
      shapeIds: this.selectedShapeIds,
      entries: [{ cell: 'LineColor', formula: `=RGB(${rgb})` }],
      forceSetf: false,
    });
    this.persistMutationSideEffects(res.wal);
    this.snapshot = res.snapshot;
    this.render();
  }

  private async commitPageRename(pageId: number, rawName: string): Promise<void> {
    this.renamingPageId = null;
    const cleaned = rawName.trim();
    if (!cleaned) {
      this.render();
      return;
    }
    const res = await this.callEngine('renamePage', { pageId, name: cleaned });
    if (res.snapshot) this.snapshot = res.snapshot;
    this.persistMutationSideEffects(res.wal);
    this.showToast(`Renamed page to "${cleaned}"`, 'info');
    this.render();
  }

  private async syncCanvasAfterDocumentOpen(): Promise<void> {
    this.renamingPageId = null;
    this.editingShapeId = null;
    if (this.wgpuAttached && this.snapshot) {
      const dpi = window.devicePixelRatio || 1;
      await this.callEngine('resizeOffscreenCanvas', {
        width: Math.round(this.snapshot.pageWidthIn * 96 * dpi),
        height: Math.round(this.snapshot.pageHeightIn * 96 * dpi),
        dpi,
      });
    }
    this.zoomPct = this.computeFitZoomPct();
  }

  private async commitDocTitleRename(rawTitle: string): Promise<void> {
    this.isRenamingDocTitle = false;
    const cleaned = rawTitle.trim();
    if (!cleaned) {
      this.render();
      return;
    }
    const normalized = /\.[a-z0-9]+$/i.test(cleaned) ? cleaned : `${cleaned}.vsdx`;
    this.currentFileName = normalized;
    try {
      const res = await this.callEngine('setDocumentTitle', { title: normalized });
      if (res.snapshot) this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
    } catch {
      // Keep local rename even if worker busy
    }
    this.showToast(`Renamed document to "${normalized}"`, 'info');
    this.render();
  }

  private async applyRichTextPatch(patch: {
    fontFamily?: string;
    fontSizePt?: number;
    bold?: boolean;
    italic?: boolean;
    underline?: boolean;
    strikethrough?: boolean;
    horizAlign?: number;
    vertAlign?: number;
    bullet?: number;
    colorHex?: string;
    bgColorHex?: string;
  }): Promise<void> {
    if (patch.fontFamily !== undefined) {
      this.textFontFamily = patch.fontFamily;
      void loadChromeOsFontBytes(patch.fontFamily).then(async (fontBytes) => {
        if (fontBytes && fontBytes.byteLength > 0) {
          const copy = new Uint8Array(fontBytes);
          const r = await this.callEngine(
            'registerFontBytes',
            { bytes: copy, familyOverride: patch.fontFamily },
            [copy.buffer]
          );
          if (r?.snapshot) {
            this.snapshot = r.snapshot;
            this.render();
          }
        }
      });
    }
    if (patch.fontSizePt !== undefined) {
      this.textFontSizePt = Math.max(6, Math.min(96, Math.round(patch.fontSizePt)));
    }
    if (patch.bold !== undefined) this.textBold = patch.bold;
    if (patch.italic !== undefined) this.textItalic = patch.italic;
    if (patch.underline !== undefined) this.textUnderline = patch.underline;
    if (patch.strikethrough !== undefined) this.textStrikethrough = patch.strikethrough;
    if (patch.horizAlign !== undefined) this.textHorizAlign = patch.horizAlign;
    if (patch.vertAlign !== undefined) this.textVertAlign = patch.vertAlign;
    if (patch.bullet !== undefined) this.textBullets = patch.bullet > 0;
    if (patch.colorHex !== undefined) this.textColorHex = patch.colorHex;
    if (patch.bgColorHex !== undefined) this.textBgColorHex = patch.bgColorHex;

    if (this.selectedShapeIds.length === 0) {
      this.render();
      return;
    }

    const fullPatch: Record<string, unknown> = { ...patch };
    if (
      patch.bold !== undefined ||
      patch.italic !== undefined ||
      patch.underline !== undefined ||
      patch.strikethrough !== undefined
    ) {
      fullPatch.bold = this.textBold;
      fullPatch.italic = this.textItalic;
      fullPatch.underline = this.textUnderline;
      fullPatch.strikethrough = this.textStrikethrough;
    }
    if (patch.fontSizePt !== undefined) {
      fullPatch.fontSizePt = this.textFontSizePt;
    }

    const res = await this.callEngine('applyTextStyle', {
      shapeIds: this.selectedShapeIds,
      patch: fullPatch,
    });
    if (res.snapshot) this.snapshot = res.snapshot;
    this.persistMutationSideEffects(res.wal);
    this.render();
  }

  private async groupSelectedShapes(): Promise<void> {
    if (this.selectedShapeIds.length < 2) {
      this.showToast('Select 2 or more shapes to group (Ctrl+G).', 'info');
      return;
    }
    const res = await this.callEngine('groupShapes', { shapeIds: this.selectedShapeIds });
    if (res.groupId > 0) {
      this.snapshot = res.snapshot;
      this.setSingleSelection(res.groupId);
      this.persistMutationSideEffects(res.wal);
      this.showToast(`Grouped into Sheet.${res.groupId}`, 'info');
      this.render();
    }
  }

  private async ungroupSelectedShapes(): Promise<void> {
    if (this.selectedShapeIds.length === 0) return;
    const res = await this.callEngine('ungroupShapes', { groupIds: this.selectedShapeIds });
    if (Array.isArray(res.ungroupedIds) && res.ungroupedIds.length > 0) {
      this.snapshot = res.snapshot;
      this.selectedShapeIds = res.ungroupedIds;
      this.persistMutationSideEffects(res.wal);
      this.showToast(`Ungrouped ${res.ungroupedIds.length} member shape(s)`, 'info');
      this.render();
    } else {
      this.showToast('Select a grouped shape to ungroup (Ctrl+Shift+G).', 'info');
    }
  }

  private findNearestPortOnPage(
    xIn: number,
    yIn: number,
    excludeShapeId = 0,
    maxDistIn = 0.24
  ): { shapeId: number; xIn: number; yIn: number; portCell: string } | null {
    if (!this.snapshot) return null;
    let best: { shapeId: number; xIn: number; yIn: number; portCell: string; dist: number } | null = null;
    for (const sh of this.snapshot.shapes) {
      if (sh.kind === 'Connector1D' || sh.id === excludeShapeId) continue;
      const ports =
        sh.ports && sh.ports.length > 0
          ? sh.ports
          : ([
              [sh.pinX - sh.width * 0.5, sh.pinY],
              [sh.pinX + sh.width * 0.5, sh.pinY],
              [sh.pinX, sh.pinY - sh.height * 0.5],
              [sh.pinX, sh.pinY + sh.height * 0.5],
            ] as Array<[number, number]>);
      const cells = sh.portCells ?? ['Connections.X1', 'Connections.X2', 'Connections.X3', 'Connections.X4'];
      for (let i = 0; i < ports.length; i++) {
        const [px, py] = ports[i];
        const d = Math.hypot(xIn - px, yIn - py);
        if (d <= maxDistIn && (!best || d < best.dist)) {
          best = {
            shapeId: sh.id,
            xIn: px,
            yIn: py,
            portCell: cells[i] ?? `Connections.X${i + 1}`,
            dist: d,
          };
        }
      }
    }
    return best ? { shapeId: best.shapeId, xIn: best.xIn, yIn: best.yIn, portCell: best.portCell } : null;
  }

  private async rotateOrFlipSelected(
    deltaAngleDeg = 0,
    toggleFlipX = false,
    toggleFlipY = false
  ): Promise<void> {
    if (this.selectedShapeIds.length === 0) return;
    const res = await this.callEngine('rotateFlipShapes', {
      shapeIds: this.selectedShapeIds,
      deltaAngleDeg: -deltaAngleDeg,
      toggleFlipX,
      toggleFlipY,
    });
    if (res.snapshot) this.snapshot = res.snapshot;
    this.persistMutationSideEffects(res.wal);
    this.render();
  }

  private compareSemVer(candidate: string, current: string): number {
    if (!candidate || candidate === current) return 0;
    const [candCore, candPre] = candidate.split('-', 2);
    const [currCore, currPre] = current.split('-', 2);
    const candNums = candCore.split('.').map((n) => Number.parseInt(n, 10) || 0);
    const currNums = currCore.split('.').map((n) => Number.parseInt(n, 10) || 0);
    const maxLen = Math.max(candNums.length, currNums.length);
    for (let i = 0; i < maxLen; i++) {
      const a = candNums[i] ?? 0;
      const b = currNums[i] ?? 0;
      if (a !== b) return a > b ? 1 : -1;
    }
    if (!candPre && currPre) return 1;
    if (candPre && !currPre) return -1;
    if (candPre && currPre && candPre !== currPre) return candPre > currPre ? 1 : -1;
    return 0;
  }

  private async applyPagePresetById(presetId: string, resetBlank = false): Promise<void> {
    const preset = PAGE_PRESET_OPTIONS.find((p) => p.id === presetId) ?? PAGE_PRESET_OPTIONS[0];
    this.activePagePresetId = preset.id;
    this.metricUnits = preset.isMetric;
    try {
      localStorage.setItem('vexio.metric', String(this.metricUnits));
    } catch {
      // Ignore storage block
    }
    if (resetBlank) {
      await this.callEngine('loadStarterTemplate', { templateId: 'blank' });
      this.currentFileName = preset.isMetric ? 'ISO-Architecture-Diagram.vsdx' : 'Architecture-Diagram.vsdx';
      this.setSingleSelection(null);
    }
    let res = await this.callEngine('setPagePreset', {
      presetSlug: preset.slug,
      landscape: preset.landscape,
    });
    if (!res.applied) {
      res = await this.callEngine('setPageDimensions', {
        widthIn: preset.widthIn,
        heightIn: preset.heightIn,
        jumpStyle: 0,
      });
    }
    this.snapshot = res.snapshot;
    if (this.wgpuAttached && this.snapshot) {
      const dpi = window.devicePixelRatio || 1;
      await this.callEngine('resizeOffscreenCanvas', {
        width: Math.round(this.snapshot.pageWidthIn * 96 * dpi),
        height: Math.round(this.snapshot.pageHeightIn * 96 * dpi),
        dpi,
      });
    }
    this.activeModal = null;
    this.zoomPct = this.computeFitZoomPct();
    this.persistMutationSideEffects(res.wal);
    this.showToast(`Page setup: ${preset.label}`, 'info');
    this.render();
  }

  private async checkForUpdates(userInitiated: boolean): Promise<void> {
    if (!userInitiated && !this.autoCheckUpdates) return;
    if (userInitiated) {
      this.updateStatusText = 'Checking Service Worker & GitHub Releases…';
      this.activeModal = 'updates';
      this.render();
    }

    if ('serviceWorker' in navigator) {
      navigator.serviceWorker
        .getRegistration()
        .then((reg) => reg?.update())
        .catch(() => {});
    }

    if (!navigator.onLine) {
      this.updateStatusText = 'Offline mode active • Running 100% locally (update check skipped cleanly).';
      if (userInitiated) this.render();
      return;
    }

    const currentVersion = '0.3.0-alpha';
    try {
      const resp = await fetch('https://api.github.com/repos/petrosvassiliou/vexio/releases', {
        headers: { Accept: 'application/vnd.github+json' },
      });
      if (!resp.ok) {
        this.updateStatusText = `Vexio v${currentVersion} is active (GitHub status ${resp.status}).`;
        if (userInitiated) this.render();
        return;
      }
      const releases = (await resp.json()) as Array<{
        tag_name?: string;
        html_url?: string;
      }>;
      const latest = releases[0];
      const tag = latest?.tag_name ?? `v${currentVersion}`;
      const cleanTag = tag.replace(/^[vV]/, '');
      this.latestReleaseUrl = latest?.html_url ?? this.latestReleaseUrl;
      const hasNewer = this.compareSemVer(cleanTag, currentVersion) > 0;
      this.latestReleaseTag = hasNewer ? tag : null;
      this.updateStatusText = hasNewer
        ? `New release available: ${tag} (Installed: v${currentVersion})`
        : `Vexio is up to date (${tag}).`;
      if (hasNewer || userInitiated) {
        this.activeModal = 'updates';
        this.render();
      }
    } catch {
      this.updateStatusText = 'Offline / firewalled network • Vexio continues 100% offline.';
      if (userInitiated) this.render();
    }
  }

  private dragState:
    | null
    | {
        mode:
          | 'pan'
          | 'move'
          | 'lasso'
          | 'resize'
          | 'rotate'
          | 'connect'
          | 'conn_endpoint'
          | 'conn_segment'
          | 'primitive'
          | 'line'
          | 'arc'
          | 'freeform'
          | 'ink'
          | 'control'
          | 'vertex'
          | 'open_text_edit';
        primitiveKind?: string;
        startPageIn: [number, number];
        curPageIn: [number, number];
        panStartClient?: [number, number];
        panStartScroll?: [number, number];
        shapeId?: number;
        connIsBegin?: boolean;
        segIndex?: number;
        segAxis?: 'x' | 'y';
        origPin?: [number, number];
        origSize?: [number, number];
        origAngleDeg?: number;
        resizeHandle?: ResizeHandleDir;
        controlRowName?: string;
        autoConnectDir?: string;
        geomIndex?: number;
        vertexRowIx?: number;
        freeformPoints?: Array<[number, number]>;
        inkPoints?: Array<{ x: number; y: number; pressure: number; tilt_x: number; tilt_y: number }>;
      } = null;

  constructor() {
    this.wgpuCanvasEl = document.createElement('canvas');
    this.wgpuCanvasEl.className = 'wgpu-viewport-canvas';
    this.wgpuCanvasEl.width = 1056;
    this.wgpuCanvasEl.height = 816;

    this.engineWorker = new Worker(new URL('./workers/engine.worker.ts', import.meta.url), {
      type: 'module',
    });
    this.opfsWorker = new Worker(new URL('./workers/opfs-journal.worker.ts', import.meta.url), {
      type: 'module',
    });

    this.engineWorker.onmessage = (ev) => {
      const { id, ok, error, ...rest } = ev.data;
      const waiter = this.pendingRpc.get(id);
      if (waiter) {
        this.pendingRpc.delete(id);
        ok ? waiter.resolve(rest) : waiter.reject(new Error(error));
      }
    };

    this.opfsWorker.onmessage = (ev) => {
      const { id, ok, error, ...rest } = ev.data;
      const waiter = this.pendingOpfs.get(id);
      if (waiter) {
        this.pendingOpfs.delete(id);
        ok ? waiter.resolve(rest) : waiter.reject(new Error(error));
      }
    };
  }

  private showToast(text: string, level: 'info' | 'warn' | 'error' = 'info'): void {
    this.toastBanner = { text, level };
    if (this.toastTimer !== null) {
      window.clearTimeout(this.toastTimer);
    }
    const existing = document.getElementById('vexio-toast-banner');
    if (existing) {
      existing.className = `vexio-toast-banner ${level === 'info' ? '' : level}`;
      existing.textContent = text;
    } else {
      this.render();
    }
    this.toastTimer = window.setTimeout(() => {
      this.toastBanner = null;
      document.getElementById('vexio-toast-banner')?.remove();
    }, 3400);
  }

  private shapeHasBelowCaption(sh: EngineSnapshot['shapes'][number]): boolean {
    const txtPinYCell = sh.cells?.find(
      (c) => c.name === 'TxtPinY' || c.name.endsWith('.TxtPinY')
    );
    if (txtPinYCell) {
      const f = txtPinYCell.formula.trim().replace(/^=/, '').trim();
      const v = txtPinYCell.value.trim();
      return f.startsWith('-') || v.startsWith('-');
    }
    // Fallback if TxtPinY fell past the 80-cell summary slice on multi-port stencils
    return sh.kind !== 'Connector1D' && sh.text.trim().length > 0 && sh.width <= 1.9 && sh.height <= 1.6;
  }

  private openInlineShapeTextEditor(shapeId: number, initialOverride?: string): void {
    const sh = this.snapshot?.shapes.find((s) => s.id === shapeId);
    if (!sh) return;
    this.setSingleSelection(shapeId);
    this.editingShapeId = sh.id;
    this.inlineTextDraft = initialOverride !== undefined ? initialOverride : sh.text;
    this.pendingInlineSelectAll = true;
    this.inlineSelectAllGuardUntilMs = performance.now() + 220;
    this.render();
  }

  private hitTestSnapshotSync(
    xIn: number,
    yIn: number,
    only2D = false,
    includeCaptionBounds = false
  ): number {
    if (!this.snapshot) return 0;
    const shapes = this.snapshot.shapes;

    // Pass 1: Exact 2D shape bodies (topmost Z-order first)
    for (let i = shapes.length - 1; i >= 0; i--) {
      const sh = shapes[i];
      if (sh.kind === 'Connector1D') continue;
      const dx = xIn - sh.pinX;
      const dy = yIn - sh.pinY;
      const rad = (-sh.angleDeg * Math.PI) / 180;
      const lx = dx * Math.cos(rad) - dy * Math.sin(rad);
      const ly = dx * Math.sin(rad) + dy * Math.cos(rad);
      if (Math.abs(lx) <= sh.width * 0.5 + 0.05 && Math.abs(ly) <= sh.height * 0.5 + 0.05) {
        return sh.id;
      }
    }

    // Pass 2: External below-icon stencil captions (when includeCaptionBounds = true)
    if (includeCaptionBounds) {
      for (let i = shapes.length - 1; i >= 0; i--) {
        const sh = shapes[i];
        if (sh.kind === 'Connector1D' || !this.shapeHasBelowCaption(sh)) continue;
        const dx = xIn - sh.pinX;
        const dy = yIn - sh.pinY;
        const rad = (-sh.angleDeg * Math.PI) / 180;
        const lx = dx * Math.cos(rad) - dy * Math.sin(rad);
        const ly = dx * Math.sin(rad) + dy * Math.cos(rad);
        const capHalfW = Math.max(sh.width * 0.5 + 0.16, 0.95);
        if (
          Math.abs(lx) <= capHalfW &&
          ly >= -sh.height * 0.5 - 0.46 &&
          ly <= -sh.height * 0.5 + 0.08
        ) {
          return sh.id;
        }
      }
    }

    // Pass 3: 1D Connectors (unless only2D = true)
    if (!only2D) {
      for (let i = shapes.length - 1; i >= 0; i--) {
        const sh = shapes[i];
        if (sh.kind !== 'Connector1D') continue;
        const pts =
          sh.pathPoints && sh.pathPoints.length >= 2
            ? sh.pathPoints
            : [
                [sh.beginX ?? sh.pinX - sh.width * 0.5, sh.beginY ?? sh.pinY] as [number, number],
                [sh.endX ?? sh.pinX + sh.width * 0.5, sh.endY ?? sh.pinY] as [number, number],
              ];
        for (let seg = 0; seg + 1 < pts.length; seg++) {
          const d = pointToSegmentDist(xIn, yIn, pts[seg][0], pts[seg][1], pts[seg + 1][0], pts[seg + 1][1]);
          if (d <= 0.14) {
            return sh.id;
          }
        }
      }
    }

    return 0;
  }

  private computeFitZoomPct(): number {
    if (!this.snapshot) return 85;
    const vp = document.querySelector('.canvas-viewport') as HTMLElement | null;
    const availW = (vp?.clientWidth ?? 860) - 64;
    const availH = (vp?.clientHeight ?? 620) - 64;
    const sheetW = this.snapshot.pageWidthIn * 96;
    const sheetH = this.snapshot.pageHeightIn * 96;
    const fit = Math.floor(Math.min(availW / Math.max(1, sheetW), availH / Math.max(1, sheetH)) * 100);
    return Math.max(25, Math.min(400, fit));
  }

  private get primarySelectedId(): number | null {
    return this.selectedShapeIds[0] ?? null;
  }

  private setSingleSelection(id: number | null): void {
    this.selectedShapeIds = id ? [id] : [];
    if (this.editingShapeId && this.editingShapeId !== id) {
      this.editingShapeId = null;
    }
    if (this.showVertexGrips && id) {
      this.refreshActiveVertices(id);
    } else {
      this.activeShapeVertices = [];
    }
  }

  private async refreshActiveVertices(shapeId: number): Promise<void> {
    try {
      const res = await this.callEngine('listShapeVertices', { shapeId });
      this.activeShapeVertices = res.vertices ?? [];
      this.render();
    } catch {
      this.activeShapeVertices = [];
    }
  }

  private callEngine(cmd: string, payload?: Record<string, unknown>, transfer?: Transferable[]): Promise<any> {
    const id = this.rpcSeq++;
    return new Promise((resolve, reject) => {
      this.pendingRpc.set(id, { resolve, reject });
      this.engineWorker.postMessage({ id, cmd, payload }, transfer ?? []);
    });
  }

  private callOpfs(op: string, extra?: Record<string, unknown>, transfer?: Transferable[]): Promise<any> {
    const id = this.rpcSeq++;
    return new Promise((resolve, reject) => {
      this.pendingOpfs.set(id, { resolve, reject });
      this.opfsWorker.postMessage({ id, op, ...extra }, transfer ?? []);
    });
  }

  private persistMutationSideEffects(wal?: Uint8Array): void {
    if (wal) {
      this.callOpfs('writeWal', { docId: this.docId, bytes: wal }, [wal.buffer]).catch(() => {});
    }
    if (this.syncDebounceTimer !== null) {
      window.clearTimeout(this.syncDebounceTimer);
    }
    this.syncDebounceTimer = window.setTimeout(async () => {
      try {
        const { crdtOpsJson } = await this.callEngine('flushCrdtOps');
        if (crdtOpsJson && crdtOpsJson !== '[]') {
          await this.collabSession?.broadcastCrdtOps(crdtOpsJson);
        }
        const { bytes } = await this.callEngine('saveFormat', { ext: 'vsdx' });
        const copyForWorkspace = new Uint8Array(bytes);
        await this.callOpfs(
          'saveWorkspace',
          { docId: this.docId, title: this.currentFileName, vsdxBytes: copyForWorkspace },
          [copyForWorkspace.buffer]
        );
      } catch {
        // Ignore transient background persistence errors
      }
    }, 110);
  }

  public async boot(): Promise<void> {
    if ('serviceWorker' in navigator) {
      let swRefreshing = false;
      const hadController = Boolean(navigator.serviceWorker.controller);
      navigator.serviceWorker.addEventListener('controllerchange', () => {
        if (hadController && !swRefreshing) {
          swRefreshing = true;
          window.location.reload();
        }
      });
      navigator.serviceWorker
        .register('/sw.js', { updateViaCache: 'none' })
        .then((reg) => reg.update())
        .catch(() => {});
    }

    bindWindowControlsOverlay();
    registerChromeOsFileLaunchHandler(async (opened) => {
      this.launchedWithExternalFile = true;
      this.activeModal = null;
      this.activeFileHandle = opened.handle;
      this.currentFileName = opened.name;
      const res = await this.callEngine('openBytes', { bytes: opened.bytes, ext: opened.ext });
      this.snapshot = res.snapshot;
      await this.syncCanvasAfterDocumentOpen();
      this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
      this.render();
    });

    this.collabSession = new VexioCollabSession({
      roomId: this.docId,
      firebaseSync: this.firebaseSync,
      onPeersChanged: (peers) => {
        this.peers = peers;
        this.renderPeersOverlayOnly();
      },
      onRemoteDocSync: async (vsdxBytes) => {
        const res = await this.callEngine('openBytes', { bytes: vsdxBytes, ext: 'vsdx' });
        this.snapshot = res.snapshot;
        await this.syncCanvasAfterDocumentOpen();
        this.render();
      },
      onRemoteCrdtOps: async (crdtOpsJson) => {
        const res = await this.callEngine('applyRemoteCrdtOps', { crdtOpsJson });
        if (res.applied > 0) {
          this.snapshot = res.snapshot;
          this.render();
        }
      },
    });

    this.firebaseSync.subscribe((st) => {
      this.cloudState = st;
      this.collabSession?.rebindFirestoreRoom();
      this.renderStatusChipsOnly();
    });
    this.firebaseSync.initNonBlocking();

    const sharedBuffer =
      typeof SharedArrayBuffer !== 'undefined' && window.crossOriginIsolated
        ? new SharedArrayBuffer(4096)
        : undefined;

    const initW = this.metricUnits ? 11.6929 : 11.0;
    const initH = this.metricUnits ? 8.2677 : 8.5;
    document.documentElement.dataset.theme = this.uiTheme;
    document.documentElement.lang = this.lang === 'gr' ? 'el' : this.lang;
    if (!this.snapshot) {
      this.snapshot = {
        title: 'Untitled Drawing',
        activePageId: 1,
        pageWidthIn: initW,
        pageHeightIn: initH,
        pages: [{ id: 1, name: 'Page-1', widthIn: initW, heightIn: initH, shapeCount: 0, isBackground: false }],
        layers: [],
        comments: [],
        shapes: [],
        svgPreview: '<svg viewBox="0 0 1122 794" xmlns="http://www.w3.org/2000/svg"><rect width="1122" height="794" fill="#FFFFFF"/></svg>',
        canUndo: false,
        canRedo: false,
        wgpuSurfaceActive: false,
        validationIssues: [],
        crossOriginIsolated: Boolean(window.crossOriginIsolated),
        webgpuAvailable: 'gpu' in navigator,
      };
      this.render();
    }

    this.bindKeyboardShortcuts();

    try {
      const initRes = await this.callEngine('init', {
        sharedBuffer,
        seedDemo: true,
        defaultPresetSlug: this.metricUnits ? 'a4' : 'letter',
        landscape: true,
      });
      this.snapshot = initRes.snapshot;
      if (initRes.stencils && Object.keys(initRes.stencils).length > 0) {
        this.stencilCatalog = { ...FALLBACK_STENCILS, ...initRes.stencils };
      }
      if (initRes.stencilPreviews && Object.keys(initRes.stencilPreviews).length > 0) {
        this.stencilPreviews = initRes.stencilPreviews;
      }
      if (Array.isArray(initRes.packOrder) && initRes.packOrder.length >= Object.keys(FALLBACK_STENCILS).length) {
        this.packOrder = initRes.packOrder;
      } else {
        this.packOrder = Object.keys(this.stencilCatalog);
      }
      const initialShapeId = this.snapshot?.shapes[1]?.id ?? this.snapshot?.shapes[0]?.id ?? null;
      this.setSingleSelection(initialShapeId);
    } catch (err) {
      this.showToast(
        `Engine notice: ${err instanceof Error ? err.message : 'initializing offline workspace'}`,
        'warn'
      );
    }

    // Preload recent OPFS workspaces, versions & cached custom stencils (100% offline-safe)
    this.callOpfs('listStencils')
      .then(async (sRes) => {
        const names: string[] = sRes?.stencils ?? [];
        for (const stName of names) {
          try {
            const r = await this.callOpfs('readStencil', { name: stName });
            if (r?.bytes && r.bytes.byteLength > 0) {
              const imp = await this.callEngine(
                'importStencil',
                { fileName: stName, bytes: r.bytes },
                [r.bytes.buffer]
              );
              if (imp.packTitle && Array.isArray(imp.masterNames) && imp.masterNames.length > 0) {
                this.stencilCatalog = { [imp.packTitle]: imp.masterNames, ...this.stencilCatalog };
                this.stencilPreviews = { [imp.packTitle]: imp.previews ?? [], ...this.stencilPreviews };
                this.packOrder = [imp.packTitle, ...this.packOrder.filter((p) => p !== imp.packTitle)];
              }
            }
          } catch {
            // Ignore unreadable cached stencil
          }
        }
        if (names.length > 0) this.render();
      })
      .catch(() => {});

    if (!this.launchedWithExternalFile) {
      Promise.all([
        this.callOpfs('listVersions', { docId: this.docId }).catch(() => ({ versions: [] })),
        this.callOpfs('listWorkspaces').catch(() => ({ workspaces: [] })),
      ]).then(([vRes, wRes]) => {
        this.opfsVersions = vRes.versions ?? [];
        this.opfsWorkspaces = wRes.workspaces ?? [];
        if (this.activeModal === 'backstage') {
          this.render();
        }
      });
    }

    // Transfer OffscreenCanvas to Rust wgpu WebGPU/WebGL2 worker pipeline once!
    if (!this.offscreenTransferred && 'transferControlToOffscreen' in this.wgpuCanvasEl) {
      try {
        this.offscreenTransferred = true;
        const offscreen = this.wgpuCanvasEl.transferControlToOffscreen();
        const dpi = window.devicePixelRatio || 1;
        const attachRes = await this.callEngine(
          'attachOffscreenCanvas',
          {
            canvas: offscreen,
            width: Math.round((this.snapshot?.pageWidthIn ?? initW) * 96 * dpi),
            height: Math.round((this.snapshot?.pageHeightIn ?? initH) * 96 * dpi),
            dpi,
          },
          [offscreen]
        );
        this.wgpuAttached = Boolean(attachRes.attached);
        if (attachRes.snapshot) {
          this.snapshot = attachRes.snapshot;
        }
      } catch {
        this.wgpuAttached = false;
      }
    }

    enumerateChromeOsSystemFonts().then((fonts) => {
      this.systemFonts = fonts;
      this.render();
    });

    if (this.autoCheckUpdates) {
      void this.checkForUpdates(false);
    }

    this.render();
  }

  private clipboardSpecs: Array<{
    kind: string;
    name: string;
    text: string;
    pinX: number;
    pinY: number;
    width: number;
    height: number;
    angleDeg: number;
    beginX?: number | null;
    beginY?: number | null;
    endX?: number | null;
    endY?: number | null;
    routeStyle?: number;
    endArrow?: number;
    fillFormula?: string;
    lineFormula?: string;
  }> = [];

  private captureSelectionToClipboard(): number {
    if (!this.snapshot || this.selectedShapeIds.length === 0) return 0;
    const specs: typeof this.clipboardSpecs = [];
    for (const sid of this.selectedShapeIds) {
      const sh = this.snapshot.shapes.find((s) => s.id === sid);
      if (!sh) continue;
      specs.push({
        kind: sh.kind,
        name: sh.name,
        text: sh.text,
        pinX: sh.pinX,
        pinY: sh.pinY,
        width: sh.width,
        height: sh.height,
        angleDeg: sh.angleDeg,
        beginX: sh.beginX,
        beginY: sh.beginY,
        endX: sh.endX,
        endY: sh.endY,
        routeStyle: sh.routeStyle,
        endArrow: sh.endArrow,
        fillFormula: sh.cells.find((c) => c.name === 'FillForegnd')?.formula,
        lineFormula: sh.cells.find((c) => c.name === 'LineColor')?.formula,
      });
    }
    this.clipboardSpecs = specs;
    this.clipboardShapeIds = [...this.selectedShapeIds];
    return specs.length;
  }

  private async copySelectedShapes(): Promise<void> {
    const n = this.captureSelectionToClipboard();
    if (n > 0) {
      this.showToast(`Copied ${n} shape(s)`, 'info');
    }
  }

  private async cutSelectedShapesClean(): Promise<void> {
    const n = this.captureSelectionToClipboard();
    if (n === 0) return;
    this.clipboardShapeIds = [];
    const res = await this.callEngine('deleteShape', { shapeIds: this.selectedShapeIds });
    this.persistMutationSideEffects(res.wal);
    this.snapshot = res.snapshot;
    this.setSingleSelection(null);
    this.showToast(`Cut ${n} shape(s)`, 'info');
    this.render();
  }

  private async pasteClipboardAt(targetPageIn?: [number, number]): Promise<void> {
    const liveIds = this.clipboardShapeIds.filter((id) =>
      this.snapshot?.shapes.some((s) => s.id === id)
    );
    if (liveIds.length > 0 && this.snapshot) {
      let dxIn = 0.35;
      let dyIn = -0.35;
      if (targetPageIn) {
        const refShapes = this.snapshot.shapes.filter((s) => liveIds.includes(s.id));
        if (refShapes.length > 0) {
          const avgX = refShapes.reduce((acc, s) => acc + s.pinX, 0) / refShapes.length;
          const avgY = refShapes.reduce((acc, s) => acc + s.pinY, 0) / refShapes.length;
          dxIn = targetPageIn[0] - avgX;
          dyIn = targetPageIn[1] - avgY;
        }
      }
      const res = await this.callEngine('duplicateShapes', { shapeIds: liveIds, dxIn, dyIn });
      this.selectedShapeIds = res.createdIds ?? [];
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
      return;
    }

    if (this.clipboardSpecs.length === 0) {
      this.showToast('Clipboard is empty — Copy (Ctrl+C) or Cut (Ctrl+X) a shape first.', 'info');
      return;
    }
    const avgX = this.clipboardSpecs.reduce((a, s) => a + s.pinX, 0) / this.clipboardSpecs.length;
    const avgY = this.clipboardSpecs.reduce((a, s) => a + s.pinY, 0) / this.clipboardSpecs.length;
    const dx = targetPageIn ? targetPageIn[0] - avgX : 0.35;
    const dy = targetPageIn ? targetPageIn[1] - avgY : -0.35;
    const newIds: number[] = [];
    let lastRes: any = null;
    for (const sp of this.clipboardSpecs) {
      if (sp.kind === 'Connector1D') {
        const bx = (sp.beginX ?? sp.pinX - 0.7) + dx;
        const by = (sp.beginY ?? sp.pinY) + dy;
        const ex = (sp.endX ?? sp.pinX + 0.7) + dx;
        const ey = (sp.endY ?? sp.pinY) + dy;
        lastRes = await this.callEngine('drawLine', { x1: bx, y1: by, x2: ex, y2: ey });
        if (lastRes?.shapeId) {
          newIds.push(lastRes.shapeId);
          if (sp.endArrow) {
            await this.callEngine('setCellFormula', {
              shapeId: lastRes.shapeId,
              cellName: 'EndArrow',
              formula: String(sp.endArrow),
              forceSetf: false,
            });
          }
          if (sp.text) {
            lastRes = await this.callEngine('setShapeText', { shapeId: lastRes.shapeId, text: sp.text });
          }
        }
      } else {
        const baseName = sp.name.split('.')[0] || 'Rectangle';
        lastRes = await this.callEngine('dropShape', {
          category: '',
          masterName: baseName,
          xIn: sp.pinX + dx,
          yIn: sp.pinY + dy,
        });
        if (lastRes?.shapeId) {
          const sid = lastRes.shapeId;
          newIds.push(sid);
          await this.callEngine('transformShape', {
            shapeId: sid,
            pinX: sp.pinX + dx,
            pinY: sp.pinY + dy,
            width: sp.width,
            height: sp.height,
            angleDeg: sp.angleDeg,
          });
          if (sp.fillFormula) {
            await this.callEngine('setCellFormula', {
              shapeId: sid,
              cellName: 'FillForegnd',
              formula: sp.fillFormula,
              forceSetf: false,
            });
          }
          if (sp.lineFormula) {
            await this.callEngine('setCellFormula', {
              shapeId: sid,
              cellName: 'LineColor',
              formula: sp.lineFormula,
              forceSetf: false,
            });
          }
          lastRes = await this.callEngine('setShapeText', { shapeId: sid, text: sp.text });
        }
      }
    }
    if (lastRes) {
      this.snapshot = lastRes.snapshot;
      this.selectedShapeIds = newIds;
      this.clipboardShapeIds = [...newIds];
      this.persistMutationSideEffects(lastRes.wal);
      this.render();
    }
  }

  private bindKeyboardShortcuts(): void {
    window.addEventListener('click', (ev) => {
      if (this.contextMenu) {
        const menuEl = document.getElementById('vexio-context-menu');
        if (!menuEl || !menuEl.contains(ev.target as Node)) {
          this.contextMenu = null;
          menuEl?.remove();
        }
      }
      if (this.openHeaderMenu) {
        const topBar = document.querySelector('.top-menubar');
        if (!topBar || !topBar.contains(ev.target as Node)) {
          this.openHeaderMenu = null;
          this.render();
        }
      }
    });

    window.addEventListener('keyup', (e) => {
      if (e.code === 'Space') {
        this.spaceKeyDown = false;
        document.getElementById('diagram-host')?.classList.remove('space-panning');
      }
    });

    window.addEventListener('keydown', async (e) => {
      const target = e.target as HTMLElement | null;
      const inInput = Boolean(
        target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT')
      );

      if (!inInput && e.code === 'Space') {
        this.spaceKeyDown = true;
        document.getElementById('diagram-host')?.classList.add('space-panning');
      }

      if (e.key === 'Escape') {
        if (this.openHeaderMenu !== null) {
          this.openHeaderMenu = null;
          this.render();
          return;
        }
        if (this.isRenamingDocTitle) {
          this.isRenamingDocTitle = false;
          this.render();
          return;
        }
        if (this.renamingPageId !== null) {
          this.renamingPageId = null;
          this.render();
          return;
        }
        if (this.contextMenu !== null) {
          this.contextMenu = null;
          this.render();
          return;
        }
        if (this.editingShapeId !== null) {
          this.editingShapeId = null;
          this.render();
          return;
        }
        if (this.activeModal !== null) {
          this.activeModal = null;
          this.render();
          return;
        }
        if (this.dragState) {
          this.dragState = null;
          const ov = document.getElementById('gesture-overlay-svg');
          if (ov) ov.innerHTML = '';
          return;
        }
        if (this.activeTool !== 'pointer') {
          this.activeTool = 'pointer';
          this.render();
          return;
        }
        if (this.selectedShapeIds.length > 0) {
          this.setSingleSelection(null);
          this.render();
          return;
        }
      }

      if (!inInput && e.key === 'F2' && this.primarySelectedId) {
        e.preventDefault();
        this.openInlineShapeTextEditor(this.primarySelectedId);
        return;
      }

      if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'b' && this.selectedShapeIds.length > 0) {
        e.preventDefault();
        await this.applyRichTextPatch({ bold: !this.textBold });
        return;
      }
      if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'i' && this.selectedShapeIds.length > 0) {
        e.preventDefault();
        await this.applyRichTextPatch({ italic: !this.textItalic });
        return;
      }
      if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'u' && this.selectedShapeIds.length > 0) {
        e.preventDefault();
        await this.applyRichTextPatch({ underline: !this.textUnderline });
        return;
      }
      if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l' && this.selectedShapeIds.length > 0) {
        e.preventDefault();
        await this.rotateOrFlipSelected(-90);
        return;
      }
      if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'r' && this.selectedShapeIds.length > 0) {
        e.preventDefault();
        await this.rotateOrFlipSelected(90);
        return;
      }
      if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'g') {
        e.preventDefault();
        if (e.shiftKey) {
          await this.ungroupSelectedShapes();
        } else {
          await this.groupSelectedShapes();
        }
        return;
      }

      if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        const res = await this.callEngine(e.shiftKey ? 'redo' : 'undo');
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects(res.wal);
        this.render();
      } else if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') {
        e.preventDefault();
        const res = await this.callEngine('redo');
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects(res.wal);
        this.render();
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 's') {
        e.preventDefault();
        await this.handleSaveAs('vsdx');
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        await this.handleSaveInPlace('vsdx');
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'o') {
        e.preventDefault();
        await this.handleOpenDisk();
      } else if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') {
        if (this.selectedShapeIds.length > 0) {
          e.preventDefault();
          await this.copySelectedShapes();
        }
      } else if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'x') {
        if (this.selectedShapeIds.length > 0) {
          e.preventDefault();
          await this.cutSelectedShapesClean();
        }
      } else if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'v') {
        e.preventDefault();
        await this.pasteClipboardAt();
      } else if (
        !inInput &&
        (e.ctrlKey || e.metaKey) &&
        e.key.toLowerCase() === 'd' &&
        this.selectedShapeIds.length > 0
      ) {
        e.preventDefault();
        const res = await this.callEngine('duplicateShapes', {
          shapeIds: this.selectedShapeIds,
          dxIn: 0.35,
          dyIn: -0.35,
        });
        this.selectedShapeIds = res.createdIds ?? this.selectedShapeIds;
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects(res.wal);
        this.render();
      } else if (!inInput && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
        e.preventDefault();
        this.selectedShapeIds = (this.snapshot?.shapes ?? []).map((s) => s.id);
        await this.callEngine('renderOffscreenFrame', { selectedIds: this.selectedShapeIds });
        this.render();
      } else if ((e.ctrlKey || e.metaKey) && e.key === '1') {
        e.preventDefault();
        this.activeTool = 'pointer';
        this.render();
      } else if ((e.ctrlKey || e.metaKey) && e.key === '2') {
        e.preventDefault();
        this.activeTool = 'text';
        this.render();
      } else if ((e.ctrlKey || e.metaKey) && e.key === '3') {
        e.preventDefault();
        this.activeTool = 'connector';
        this.render();
      } else if (!inInput && (e.key === 'Delete' || e.key === 'Backspace') && this.selectedShapeIds.length > 0) {
        e.preventDefault();
        const res = await this.callEngine('deleteShape', { shapeIds: this.selectedShapeIds });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.setSingleSelection(null);
        this.render();
      } else if (
        !inInput &&
        this.selectedShapeIds.length > 0 &&
        ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)
      ) {
        e.preventDefault();
        const step = e.shiftKey ? 0.025 : 0.1;
        const dxIn = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0;
        const dyIn = e.key === 'ArrowDown' ? -step : e.key === 'ArrowUp' ? step : 0;
        const res = await this.callEngine('moveShapesDelta', {
          shapeIds: this.selectedShapeIds,
          dxIn,
          dyIn,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      } else if (e.shiftKey && e.key === 'F5') {
        e.preventDefault();
        this.showShapeSheet = !this.showShapeSheet;
        this.render();
      } else if (!inInput && !e.ctrlKey && !e.metaKey && !e.altKey) {
        const keyMap: Record<string, ActiveTool> = {
          v: 'pointer',
          h: 'pan',
          l: 'line',
          a: 'arc',
          c: 'connector',
          r: 'rect',
          u: 'round_rect',
          o: 'ellipse',
          d: 'diamond',
          g: 'triangle',
          x: 'hexagon',
          s: 'star',
          p: 'freeform',
          t: 'text',
          i: 'pen',
        };
        const mapped = keyMap[e.key.toLowerCase()];
        if (mapped) {
          this.activeTool = mapped;
          this.render();
        }
      }
    });
  }

  private async openBackstageModal(): Promise<void> {
    this.openHeaderMenu = null;
    this.activeModal = 'backstage';
    this.modalOutputMessage = '';
    try {
      const [vRes, wRes, cwRes] = await Promise.all([
        this.callOpfs('listVersions', { docId: this.docId }),
        this.callOpfs('listWorkspaces'),
        this.firebaseSync.listCloudWorkspaces(),
      ]);
      this.opfsVersions = vRes.versions ?? [];
      this.opfsWorkspaces = wRes.workspaces ?? [];
      this.cloudWorkspaces = cwRes;
    } catch {
      // Offline OPFS fallback
    }
    this.render();
  }

  private async handleOpenDisk(): Promise<void> {
    this.openHeaderMenu = null;
    const opened = await openLocalDiagramFile();
    if (!opened) return;
    if (['vssx', 'vssm', 'vss', 'vst'].includes(opened.ext)) {
      const opfsCopy = new Uint8Array(opened.bytes);
      this.callOpfs('cacheStencil', { name: opened.name, vssxBytes: opfsCopy }, [opfsCopy.buffer]).catch(() => {});
      const res = await this.callEngine(
        'importStencil',
        { fileName: opened.name, bytes: opened.bytes },
        [opened.bytes.buffer]
      );
      if (res.packTitle && Array.isArray(res.masterNames) && res.masterNames.length > 0) {
        this.stencilCatalog = { [res.packTitle]: res.masterNames, ...this.stencilCatalog };
        this.stencilPreviews = { [res.packTitle]: res.previews ?? [], ...this.stencilPreviews };
        this.packOrder = [res.packTitle, ...this.packOrder.filter((p) => p !== res.packTitle)];
        this.expandedStencilPacks.add(res.packTitle);
        this.stencilGroupFilter = 'all';
        this.activeModal = null;
        this.showToast(`Imported Visio stencil "${res.packTitle}" (${res.masterNames.length} masters)`, 'info');
        this.render();
      }
      return;
    }
    this.activeFileHandle = opened.handle;
    this.currentFileName = opened.name;
    const res = await this.callEngine('openBytes', { bytes: opened.bytes, ext: opened.ext });
    this.snapshot = res.snapshot;
    this.activeModal = null;
    await this.syncCanvasAfterDocumentOpen();
    this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
    this.persistMutationSideEffects();
    this.showToast(`Opened "${opened.name}"`, 'info');
    this.render();
  }

  private async handleSaveInPlace(ext: string): Promise<void> {
    const { bytes, wal } = await this.callEngine('saveFormat', { ext });
    this.persistMutationSideEffects(wal);
    const isCurrentVsdx = /\.vsd[xm]$/i.test(this.currentFileName);
    const baseStem = this.currentFileName.replace(/\.[^.]+$/, '');
    const targetName = ext === 'vsdx' ? (isCurrentVsdx ? this.currentFileName : `${baseStem}.vsdx`) : `${baseStem}.${ext}`;
    const reusableHandle = ext === 'vsdx' && isCurrentVsdx ? this.activeFileHandle : null;
    const handle = await saveLocalDiagramInPlace(reusableHandle, targetName, bytes);
    if (ext === 'vsdx' && handle) {
      this.activeFileHandle = handle;
      this.currentFileName = handle.name || targetName;
      this.render();
    }
    if (ext === 'vsdx' && this.cloudState.user) {
      await this.firebaseSync.backupWorkspaceToCloud(this.docId, this.currentFileName, 'vsdx', bytes);
    }
    this.showToast(`Saved ${handle?.name || targetName}`, 'info');
  }

  private async handleSaveAs(ext = 'vsdx'): Promise<void> {
    this.openHeaderMenu = null;
    const { bytes, wal } = await this.callEngine('saveFormat', { ext });
    this.persistMutationSideEffects(wal);
    const baseStem = this.currentFileName.replace(/\.[^.]+$/, '') || 'Architecture-Diagram';
    const suggestedName = `${baseStem}-copy.${ext}`;
    // Always pass null existingHandle so the OS Save As dialog opens unconditionally
    const handle = await saveLocalDiagramInPlace(null, suggestedName, bytes);
    if (ext === 'vsdx' && handle) {
      this.activeFileHandle = handle;
      this.currentFileName = handle.name || suggestedName;
      if (this.cloudState.user) {
        await this.firebaseSync.backupWorkspaceToCloud(this.docId, this.currentFileName, 'vsdx', bytes);
      }
      this.render();
    }
    if (handle || !window.showSaveFilePicker) {
      this.showToast(`Saved As ${handle?.name || suggestedName}`, 'info');
    }
  }

  private clientToPageInches(hostEl: HTMLElement, clientX: number, clientY: number): [number, number] {
    const rect = hostEl.getBoundingClientRect();
    const pageW = this.snapshot?.pageWidthIn ?? 11.0;
    const pageH = this.snapshot?.pageHeightIn ?? 8.5;
    const u = Math.max(0, Math.min(1, (clientX - rect.left) / Math.max(1, rect.width)));
    const v = Math.max(0, Math.min(1, (clientY - rect.top) / Math.max(1, rect.height)));
    return [u * pageW, (1 - v) * pageH];
  }

  private renderStatusChipsOnly(): void {
    const el = document.getElementById('cloud-sync-chip');
    if (!el) return;
    el.className = `status-chip ${this.cloudState.online ? '' : 'offline'} no-drag`;
    el.textContent = this.cloudState.online
      ? this.cloudState.user
        ? 'Cloud Synced'
        : 'Saved Locally'
      : 'Offline Ready';
  }

  private renderPeersOverlayOnly(): void {
    const stackEl = document.getElementById('peer-avatar-stack');
    if (stackEl) {
      stackEl.innerHTML = this.peers
        .map(
          (p) =>
            `<span class="peer-avatar-pill" style="background:${p.color}" title="${p.peerName} (Live Co-Editing)">● ${p.peerName}</span>`
        )
        .join('');
    }
    const layerEl = document.getElementById('peer-cursor-layer');
    if (layerEl && this.snapshot) {
      const ppi = 96;
      const pageH = this.snapshot.pageHeightIn;
      const activePid = this.snapshot.activePageId;
      layerEl.innerHTML = this.peers
        .filter((p) => p.pageId === activePid)
        .map((p) => {
          const lx = p.cursorX * ppi;
          const ty = (pageH - p.cursorY) * ppi;
          return `
            <div class="peer-cursor-pin" style="left:${lx.toFixed(1)}px;top:${ty.toFixed(1)}px">
              <svg width="14" height="18" viewBox="0 0 14 18" style="display:block">
                <path d="M1 1 L13 10 L7 11 L4 17 Z" fill="${p.color}" stroke="#ffffff" stroke-width="1.4"/>
              </svg>
              <span class="peer-cursor-badge" style="background:${p.color}">${p.peerName}</span>
            </div>
          `;
        })
        .join('');
    }
  }

  private renderHorizRulerSvg(pageWidthIn: number, ppiZoom: number): string {
    const totalW = Math.ceil(pageWidthIn * ppiZoom) + 120;
    const ticks: string[] = [];
    if (this.metricUnits) {
      const totalMm = Math.ceil(pageWidthIn * 25.4);
      const steps = Math.ceil(totalMm / 5);
      for (let s = 0; s <= steps; s++) {
        const mm = s * 5;
        const x = 32 + (mm / 25.4) * ppiZoom;
        const isMajor = mm % 20 === 0;
        const isCm = mm % 10 === 0;
        const h = isMajor ? 12 : isCm ? 8 : 4;
        ticks.push(`<line x1="${x.toFixed(1)}" y1="${22 - h}" x2="${x.toFixed(1)}" y2="22" stroke="var(--m3-outline-strong)" stroke-width="1"/>`);
        if (isMajor) {
          ticks.push(`<text x="${(x + 3).toFixed(1)}" y="11" font-size="8.5" font-weight="600" fill="var(--m3-text-muted)">${mm}</text>`);
        }
      }
    } else {
      const steps = Math.ceil(pageWidthIn * 4);
      for (let q = 0; q <= steps; q++) {
        const x = 32 + q * 0.25 * ppiZoom;
        const isInch = q % 4 === 0;
        const isHalf = q % 2 === 0;
        const h = isInch ? 12 : isHalf ? 7 : 4;
        ticks.push(`<line x1="${x.toFixed(1)}" y1="${22 - h}" x2="${x.toFixed(1)}" y2="22" stroke="var(--m3-outline-strong)" stroke-width="1"/>`);
        if (isInch) {
          ticks.push(`<text x="${(x + 3).toFixed(1)}" y="11" font-size="9" font-weight="600" fill="var(--m3-text-muted)">${q / 4}</text>`);
        }
      }
    }
    return `<svg class="ruler-svg" viewBox="0 0 ${totalW} 22" preserveAspectRatio="xMinYMin slice">${ticks.join('')}</svg>`;
  }

  private renderVertRulerSvg(pageHeightIn: number, ppiZoom: number): string {
    const totalH = Math.ceil(pageHeightIn * ppiZoom) + 120;
    const ticks: string[] = [];
    if (this.metricUnits) {
      const totalMm = Math.ceil(pageHeightIn * 25.4);
      const steps = Math.ceil(totalMm / 5);
      for (let s = 0; s <= steps; s++) {
        const mm = s * 5;
        const y = 32 + (mm / 25.4) * ppiZoom;
        const isMajor = mm % 20 === 0;
        const isCm = mm % 10 === 0;
        const w = isMajor ? 12 : isCm ? 8 : 4;
        ticks.push(`<line x1="${24 - w}" y1="${y.toFixed(1)}" x2="24" y2="${y.toFixed(1)}" stroke="var(--m3-outline-strong)" stroke-width="1"/>`);
        if (isMajor) {
          const label = Math.max(0, Math.round(pageHeightIn * 25.4 - mm));
          ticks.push(`<text x="2" y="${(y + 9).toFixed(1)}" font-size="8" font-weight="600" fill="var(--m3-text-muted)">${label}</text>`);
        }
      }
    } else {
      const steps = Math.ceil(pageHeightIn * 4);
      for (let q = 0; q <= steps; q++) {
        const y = 32 + q * 0.25 * ppiZoom;
        const isInch = q % 4 === 0;
        const isHalf = q % 2 === 0;
        const w = isInch ? 12 : isHalf ? 7 : 4;
        ticks.push(`<line x1="${24 - w}" y1="${y.toFixed(1)}" x2="24" y2="${y.toFixed(1)}" stroke="var(--m3-outline-strong)" stroke-width="1"/>`);
        if (isInch) {
          const label = Math.max(0, Math.round(pageHeightIn - q / 4));
          ticks.push(`<text x="3" y="${(y + 9).toFixed(1)}" font-size="9" font-weight="600" fill="var(--m3-text-muted)">${label}</text>`);
        }
      }
    }
    return `<svg class="ruler-svg" viewBox="0 0 24 ${totalH}" preserveAspectRatio="xMinYMin slice">${ticks.join('')}</svg>`;
  }

  private getStencilSvgPreview(packName: string, masterName: string): string {
    const list = this.stencilPreviews[packName];
    const found = list?.find((item) => item.name === masterName);
    if (found?.svg) {
      return found.svg;
    }
    return buildFallbackStencilSvg(masterName);
  }

  private shortPackLabel(packName: string): string {
    if (packName === 'Basic Shapes') return 'Basic Shapes';
    if (packName.startsWith('Microsoft 365')) return 'M365 & Office';
    if (packName.startsWith('Microsoft Servers')) return 'MS Servers & AD';
    if (packName.startsWith('Microsoft Azure')) return 'Microsoft Azure';
    if (packName.startsWith('Microsoft Power Platform')) return 'Power Platform & D365';
    if (packName.startsWith('Arrows')) return 'Arrows';
    if (packName.startsWith('Callouts')) return 'Callouts & Notes';
    if (packName.startsWith('Cross-Functional')) return 'Swimlanes';
    if (packName.startsWith('Cloud Architecture')) return 'Cloud (AWS/GCP/Azure)';
    if (packName.startsWith('Network & Rack')) return 'Network & Rack';
    if (packName.startsWith('Floor Plan')) return 'Floor Plan';
    if (packName.startsWith('P&ID')) return 'P&ID Engineering';
    if (packName.startsWith('Electrical')) return 'Electrical (IEEE)';
    if (packName.startsWith('UI & Application')) return 'UI Wireframes';
    if (packName.startsWith('Organization')) return 'Org Chart';
    if (packName.startsWith('Project Schedule')) return 'Gantt & Timeline';
    if (packName.startsWith('Value Stream')) return 'Lean VSM';
    if (packName.startsWith('Database')) return 'Database / ERD';
    return packName;
  }

  private renderDrawingToolRail(): string {
    const btn = (tool: ActiveTool, icon: string, label: string, tip: string) =>
      `<button class="rail-tool-btn ${this.activeTool === tool ? 'active' : ''}" data-tool="${tool}" title="${tip}">
        ${icon}
        <span class="rail-tool-label">${label}</span>
      </button>`;

    return `
      <aside class="drawing-tool-rail" aria-label="Drawing Tools">
        ${btn('pointer', UI_ICONS.pointer, 'Select', 'Pointer & Marquee Select (V / Ctrl+1)')}
        ${btn('pan', UI_ICONS.pan, 'Pan', 'Pan Canvas (H / Hold Space)')}
        <div class="rail-divider"></div>
        ${btn('connector', UI_ICONS.connector, 'Connect', 'Dynamic Connector — Drag between shapes or ports (C / Ctrl+3)')}
        ${btn('line', UI_ICONS.line, 'Line', 'Straight Line — Click and drag on canvas (L)')}
        ${btn('arc', UI_ICONS.arc, 'Arc', 'Curved Arc — Click and drag on canvas (A)')}
        ${btn('freeform', UI_ICONS.freeform, 'Curve', 'Freeform Bezier / Spline Curve (P)')}
        <div class="rail-divider"></div>
        ${btn('rect', UI_ICONS.rect, 'Rect', 'Rectangle / Square — Click and drag on canvas (R)')}
        ${btn('round_rect', UI_ICONS.round_rect, 'Round', 'Rounded Rectangle — Click and drag on canvas (U)')}
        ${btn('ellipse', UI_ICONS.ellipse, 'Circle', 'Ellipse / Circle — Click and drag on canvas (O)')}
        ${btn('diamond', UI_ICONS.diamond, 'Diamond', 'Diamond / Decision — Click and drag on canvas (D)')}
        ${btn('triangle', UI_ICONS.triangle, 'Triangle', 'Triangle — Click and drag on canvas (G)')}
        ${btn('hexagon', UI_ICONS.hexagon, 'Hexagon', 'Hexagon / Polygon — Click and drag on canvas (X)')}
        ${btn('star', UI_ICONS.star, 'Star', '5-Point Star — Click and drag on canvas (S)')}
        <div class="rail-divider"></div>
        ${btn('text', UI_ICONS.text, 'Text', 'Text Box — Click or drag on canvas to type immediately, or double-click any stencil (T)')}
        ${btn('pen', UI_ICONS.pen, 'Pen', 'Freehand Markup Ink Pen (I)')}
        ${btn('highlighter', UI_ICONS.highlighter, 'Marker', 'Markup Highlighter')}
      </aside>
    `;
  }

  private renderContextMenu(snap: EngineSnapshot): string {
    if (!this.contextMenu) return '';
    const cm = this.contextMenu;
    const clampedLeft = Math.max(8, Math.min(window.innerWidth - 260, cm.x));
    const clampedTop = Math.max(8, Math.min(window.innerHeight - 400, cm.y));
    const hasClip = this.clipboardShapeIds.length > 0 || this.clipboardSpecs.length > 0;
    const t = this.i18n;

    if (cm.kind === 'shape') {
      const sh = this.primarySelectedId ? snap.shapes.find((s) => s.id === this.primarySelectedId) : null;
      const is1D = sh?.kind === 'Connector1D';
      const rs = sh?.routeStyle ?? 16;
      const ea = sh?.endArrow ?? 4;
      return `
        <div id="vexio-context-menu" class="vexio-context-menu" style="left:${clampedLeft}px;top:${clampedTop}px">
          <div class="ctx-header">${sh ? `${sh.text || sh.name} (Sheet.${sh.id})` : `${this.selectedShapeIds.length} Shapes Selected`}</div>
          <button class="ctx-item" data-ctx="edit-text"><span>${t.editLabel}</span><span class="ctx-kbd">F2</span></button>
          <div class="ctx-sep"></div>
          <button class="ctx-item" data-ctx="rotate-left"><span>${t.rotateLeft90}</span><span class="ctx-kbd">Ctrl+L</span></button>
          <button class="ctx-item" data-ctx="rotate-right"><span>${t.rotateRight90}</span><span class="ctx-kbd">Ctrl+R</span></button>
          <button class="ctx-item" data-ctx="flip-h"><span>${t.flipHoriz}</span></button>
          <button class="ctx-item" data-ctx="flip-v"><span>${t.flipVert}</span></button>
          <div class="ctx-sep"></div>
          <button class="ctx-item" data-ctx="cut"><span>Cut</span><span class="ctx-kbd">Ctrl+X</span></button>
          <button class="ctx-item" data-ctx="copy"><span>Copy</span><span class="ctx-kbd">Ctrl+C</span></button>
          <button class="ctx-item" data-ctx="paste" ${hasClip ? '' : 'disabled'}><span>Paste</span><span class="ctx-kbd">Ctrl+V</span></button>
          <button class="ctx-item" data-ctx="duplicate"><span>Duplicate</span><span class="ctx-kbd">Ctrl+D</span></button>
          <button class="ctx-item danger" data-ctx="delete"><span>Delete</span><span class="ctx-kbd">Del</span></button>
          <div class="ctx-sep"></div>
          <button class="ctx-item" data-ctx="bring-front"><span>Bring to Front</span><span class="ctx-kbd">Top</span></button>
          <button class="ctx-item" data-ctx="send-back"><span>Send to Back</span><span class="ctx-kbd">Back</span></button>
          <button class="ctx-item" data-ctx="group" ${this.selectedShapeIds.length >= 2 ? '' : 'disabled'}><span>Group</span><span class="ctx-kbd">Ctrl+G</span></button>
          <button class="ctx-item" data-ctx="ungroup" ${sh?.kind === 'Group' || snap.shapes.some((s) => this.selectedShapeIds.includes(s.id) && s.kind === 'Group') ? '' : 'disabled'}><span>Ungroup</span><span class="ctx-kbd">Ctrl+Shift+G</span></button>
          ${
            is1D
              ? `
                <div class="ctx-sep"></div>
                <button class="ctx-item" data-ctx="route-ortho"><span>${rs !== 1 && rs !== 2 ? '✓ ' : ''}Right-Angle Connector</span></button>
                <button class="ctx-item" data-ctx="route-curved"><span>${rs === 2 ? '✓ ' : ''}Curved Connector</span></button>
                <button class="ctx-item" data-ctx="route-straight"><span>${rs === 1 ? '✓ ' : ''}Straight Line</span></button>
                <div class="ctx-sep"></div>
                <button class="ctx-item" data-ctx="arrow-end"><span>${ea > 0 ? '✓ ' : ''}End Arrowhead</span></button>
                <button class="ctx-item" data-ctx="arrow-both"><span>Both Ends Arrow</span></button>
                <button class="ctx-item" data-ctx="arrow-none"><span>${ea === 0 ? '✓ ' : ''}No Arrowheads</span></button>
              `
              : `
                <div class="ctx-sep"></div>
                <button class="ctx-item" data-ctx="add-callout"><span>Attach Callout Note</span></button>
                <button class="ctx-item" data-ctx="link-subprocess"><span>Link Subprocess Page...</span></button>
                <button class="ctx-item" data-ctx="open-hyperlink"><span>Hyperlink &amp; ScreenTip...</span></button>
              `
          }
          <div class="ctx-sep"></div>
          <button class="ctx-item" data-ctx="show-shapesheet"><span>Inspect ShapeSheet</span><span class="ctx-kbd">Shift+F5</span></button>
        </div>
      `;
    }

    if (cm.kind === 'page') {
      const p = snap.pages.find((pg) => pg.id === cm.targetPageId);
      return `
        <div id="vexio-context-menu" class="vexio-context-menu" style="left:${clampedLeft}px;top:${clampedTop}px">
          <div class="ctx-header">Page: ${p?.name ?? 'Page'}</div>
          <button class="ctx-item" data-ctx="page-rename"><span>${t.renamePage}</span></button>
          <button class="ctx-item" data-ctx="page-duplicate"><span>Duplicate Page</span></button>
          <button class="ctx-item" data-ctx="page-insert"><span>Insert Blank Page</span></button>
          <div class="ctx-sep"></div>
          <button class="ctx-item danger" data-ctx="page-delete" ${snap.pages.length <= 1 ? 'disabled' : ''}><span>Delete Page</span></button>
        </div>
      `;
    }

    if (cm.kind === 'stencil') {
      return `
        <div id="vexio-context-menu" class="vexio-context-menu" style="left:${clampedLeft}px;top:${clampedTop}px">
          <div class="ctx-header">${cm.stencilMaster ?? 'Stencil Shape'}</div>
          <button class="ctx-item" data-ctx="stencil-drop-center"><span>Place on Center of Page</span></button>
          <button class="ctx-item" data-ctx="stencil-drop-three"><span>Place 3 Connected Steps</span></button>
        </div>
      `;
    }

    return `
      <div id="vexio-context-menu" class="vexio-context-menu" style="left:${clampedLeft}px;top:${clampedTop}px">
        <div class="ctx-header">Canvas • (${this.formatLengthFromIn(cm.pageIn?.[0] ?? 0)}, ${this.formatLengthFromIn(cm.pageIn?.[1] ?? 0)})</div>
        <button class="ctx-item" data-ctx="paste-here" ${hasClip ? '' : 'disabled'}><span>Paste Here</span><span class="ctx-kbd">Ctrl+V</span></button>
        <button class="ctx-item" data-ctx="select-all"><span>Select All Shapes</span><span class="ctx-kbd">Ctrl+A</span></button>
        <div class="ctx-sep"></div>
        <button class="ctx-item" data-ctx="drop-here-rect"><span>Insert Rectangle Here</span><span class="ctx-kbd">R</span></button>
        <button class="ctx-item" data-ctx="drop-here-ellipse"><span>Insert Circle / Ellipse Here</span><span class="ctx-kbd">O</span></button>
        <button class="ctx-item" data-ctx="drop-here-diamond"><span>Insert Decision Diamond Here</span><span class="ctx-kbd">D</span></button>
        <button class="ctx-item" data-ctx="drop-here-text"><span>Insert Text Box Here</span><span class="ctx-kbd">T</span></button>
        <button class="ctx-item" data-ctx="drop-here-note"><span>Insert Folded Note Here</span></button>
        <div class="ctx-sep"></div>
        <button class="ctx-item" data-ctx="layout-tb"><span>Auto-Layout Page (Top-Down)</span></button>
        <button class="ctx-item" data-ctx="layout-lr"><span>Auto-Layout Page (Left-Right)</span></button>
        <div class="ctx-sep"></div>
        <button class="ctx-item" data-ctx="export-jpg"><span>Export Page as JPG (.jpg)</span></button>
        <button class="ctx-item" data-ctx="export-pdf"><span>Export Document as PDF (.pdf)</span></button>
        <div class="ctx-sep"></div>
        <button class="ctx-item" data-ctx="zoom-fit"><span>Fit Page to Window</span></button>
        <button class="ctx-item" data-ctx="toggle-grid"><span>${this.showGrid ? '✓ ' : ''}Show Dot Grid</span></button>
        <button class="ctx-item" data-ctx="toggle-rulers"><span>${this.showRulers ? '✓ ' : ''}Show Rulers</span></button>
        <button class="ctx-item" data-ctx="toggle-smart-guides"><span>${this.showSmartGuides ? '✓ ' : ''}Smart Guides</span></button>
      </div>
    `;
  }

  private getAvailableOsFontList(extraFamily?: string): string[] {
    const baseFonts = [
      'Inter',
      'Roboto',
      'Segoe UI',
      'Arial',
      'Calibri',
      'Liberation Sans',
      'Noto Sans',
      'Helvetica Neue',
      'Georgia',
      'Times New Roman',
      'JetBrains Mono',
      'Courier New',
    ];
    return Array.from(
      new Set([
        ...(extraFamily ? [extraFamily] : []),
        this.textFontFamily,
        ...baseFonts,
        ...this.systemFonts,
      ])
    ).filter((f) => Boolean(f && f.trim()));
  }

  private renderContextualRibbon(snap: EngineSnapshot): string {
    const selDisabled = this.selectedShapeIds.length === 0 ? 'disabled' : '';
    const multiDisabled = this.selectedShapeIds.length < 2 ? 'disabled' : '';
    const distDisabled = this.selectedShapeIds.length < 3 ? 'disabled' : '';
    const hasGroupSelected = snap.shapes.some(
      (s) => this.selectedShapeIds.includes(s.id) && s.kind === 'Group'
    );
    const ungroupDisabled = hasGroupSelected ? '' : 'disabled';
    const hasClip = this.clipboardShapeIds.length > 0 || this.clipboardSpecs.length > 0;
    const primarySh = this.primarySelectedId ? snap.shapes.find((s) => s.id === this.primarySelectedId) : null;
    const fontList = this.getAvailableOsFontList(primarySh?.fontFamily);
    const fontSizes = Array.from(
      new Set([this.textFontSizePt, 8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48])
    ).sort((a, b) => a - b);
    const activeTextColor = /^#[0-9a-fA-F]{6}$/.test(this.textColorHex) ? this.textColorHex : '#0F172A';
    const activeBgColor = /^#[0-9a-fA-F]{6}$/.test(this.textBgColorHex) ? this.textBgColorHex : '#FEF08A';

    switch (this.activeRibbonTab) {
      case 'Home':
        return `
          <span class="ribbon-group-label">Clipboard</span>
          <button class="m3-btn" id="btn-paste-sel" ${hasClip ? '' : 'disabled'} title="Paste (Ctrl+V)">Paste</button>
          <button class="m3-btn" id="btn-copy-sel" ${selDisabled} title="Copy (Ctrl+C)">Copy</button>
          <button class="m3-btn" id="btn-duplicate-sel" ${selDisabled} title="Duplicate Selected (Ctrl+D)">${UI_ICONS.duplicate}</button>
          <button class="m3-btn" id="btn-delete-sel" ${selDisabled} title="Delete Selected (Del)">${UI_ICONS.trash}</button>
          <div class="ribbon-divider"></div>

          <span class="ribbon-group-label">Font &amp; Paragraph</span>
          <select class="cell-input" id="ribbon-font-family" style="width:136px;height:26px;padding:1px 6px;font-size:11.5px" title="Installed OS &amp; Document Font Families (${fontList.length})">
            ${fontList
              .map(
                (f) => `<option value="${f.replace(/"/g, '&quot;')}" ${this.textFontFamily === f ? 'selected' : ''}>${f}</option>`
              )
              .join('')}
          </select>
          ${
            typeof window !== 'undefined' && 'queryLocalFonts' in window
              ? `<button class="fmt-btn" id="btn-load-os-fonts" title="${this.i18n.loadOsFonts} — Load all installed OS fonts">${this.i18n.loadOsFonts}</button>`
              : ''
          }
          <select class="cell-input" id="ribbon-font-size" style="width:58px;height:26px;padding:1px 4px;font-size:11.5px" title="Font Size (pt)">
            ${fontSizes
              .map((sz) => `<option value="${sz}" ${this.textFontSizePt === sz ? 'selected' : ''}>${sz}pt</option>`)
              .join('')}
          </select>
          <button class="fmt-btn" id="btn-font-grow" title="Grow Font Size (A▲)">A▲</button>
          <button class="fmt-btn" id="btn-font-shrink" title="Shrink Font Size (A▼)">A▼</button>
          <button class="fmt-btn ${this.textBold ? 'active' : ''}" id="btn-fmt-bold" title="Bold (Ctrl+B)"><strong>B</strong></button>
          <button class="fmt-btn ${this.textItalic ? 'active' : ''}" id="btn-fmt-italic" title="Italic (Ctrl+I)"><em>I</em></button>
          <button class="fmt-btn ${this.textUnderline ? 'active' : ''}" id="btn-fmt-underline" title="Underline (Ctrl+U)"><u>U</u></button>
          <button class="fmt-btn ${this.textStrikethrough ? 'active' : ''}" id="btn-fmt-strike" title="Strikethrough"><s>S</s></button>
          <label class="native-color-chip" title="Text Color (Native OS Color Picker)">
            <span>A</span>
            <input type="color" id="ribbon-text-color-picker" class="native-color-input" value="${activeTextColor}" />
          </label>
          <button class="fmt-btn ${this.textBgColorHex !== 'none' ? 'active' : ''}" id="btn-apply-text-bg" title="Toggle Yellow Text Highlight">${this.textBgColorHex !== 'none' ? 'ab✓' : 'ab'}</button>
          <label class="native-color-chip" title="Text Highlight Color (Native OS Color Picker)">
            <span>▾</span>
            <input type="color" id="ribbon-text-bg-picker" class="native-color-input" value="${activeBgColor}" />
          </label>
          ${
            this.textBgColorHex !== 'none'
              ? `<button class="fmt-btn" id="btn-clear-text-bg" title="${this.i18n.clearHighlight}">✕</button>`
              : ''
          }
          <button class="fmt-btn ${this.textHorizAlign === 0 ? 'active' : ''}" data-halign="0" title="Align Text Left">≡L</button>
          <button class="fmt-btn ${this.textHorizAlign === 1 ? 'active' : ''}" data-halign="1" title="Center Text">≡C</button>
          <button class="fmt-btn ${this.textHorizAlign === 2 ? 'active' : ''}" data-halign="2" title="Align Text Right">≡R</button>
          <button class="fmt-btn ${this.textVertAlign === 0 ? 'active' : ''}" data-valign="0" title="Top Vertical Align">⤒</button>
          <button class="fmt-btn ${this.textVertAlign === 1 ? 'active' : ''}" data-valign="1" title="Middle Vertical Align">↕</button>
          <button class="fmt-btn ${this.textVertAlign === 2 ? 'active' : ''}" data-valign="2" title="Bottom Vertical Align">⤓</button>
          <button class="fmt-btn ${this.textBullets ? 'active' : ''}" id="btn-fmt-bullets" title="Bulleted List">• List</button>
          <div class="ribbon-divider"></div>

          <span class="ribbon-group-label">Fill &amp; Line</span>
          <label class="native-color-chip" title="${this.i18n.fillColor} (Native OS Color Picker)">
            <span>Fill</span>
            <input type="color" id="ribbon-fill-color-picker" class="native-color-input" value="${this.shapeFillHex}" />
          </label>
          <label class="native-color-chip" title="${this.i18n.lineColor} (Native OS Color Picker)">
            <span>Line</span>
            <input type="color" id="ribbon-line-color-picker" class="native-color-input" value="${this.shapeLineHex}" />
          </label>
          <div class="swatch-row">
            ${['255,255,255', '239,246,255', '219,234,254', '220,252,231', '254,243,199', '243,232,255', '254,226,226']
              .map(
                (rgb) =>
                  `<button class="swatch-btn" data-fill-rgb="${rgb}" style="background:rgb(${rgb})" title="Fill RGB(${rgb})"></button>`
              )
              .join('')}
          </div>
          <div class="ribbon-divider"></div>

          <span class="ribbon-group-label">Rotate &amp; Arrange</span>
          <button class="m3-btn tonal" data-rot-deg="-90" ${selDisabled} title="${this.i18n.rotateLeft90} (Ctrl+L)">↺ 90°</button>
          <button class="m3-btn tonal" data-rot-deg="90" ${selDisabled} title="${this.i18n.rotateRight90} (Ctrl+R)">↻ 90°</button>
          <button class="m3-btn" data-flip-axis="x" ${selDisabled} title="${this.i18n.flipHoriz}">⇆ Flip H</button>
          <button class="m3-btn" data-flip-axis="y" ${selDisabled} title="${this.i18n.flipVert}">⇅ Flip V</button>
          <button class="m3-btn" data-zorder="front" ${selDisabled} title="Bring Selected to Front">Front</button>
          <button class="m3-btn" data-zorder="back" ${selDisabled} title="Send Selected to Back">Back</button>
          <button class="m3-btn" data-align="left" ${multiDisabled} title="Align Left">Align L</button>
          <button class="m3-btn" data-align="center" ${multiDisabled} title="Align Center">Center</button>
          <button class="m3-btn" data-align="right" ${multiDisabled} title="Align Right">Align R</button>
          <button class="m3-btn" data-align="top" ${multiDisabled} title="Align Top">Top</button>
          <button class="m3-btn" data-align="middle" ${multiDisabled} title="Align Middle">Middle</button>
          <button class="m3-btn" data-align="bottom" ${multiDisabled} title="Align Bottom">Bottom</button>
          <button class="m3-btn" data-align="distribute_h" ${distDisabled} title="Distribute Horizontally (Equal Gaps)">Dist H</button>
          <button class="m3-btn" data-align="distribute_v" ${distDisabled} title="Distribute Vertically (Equal Gaps)">Dist V</button>
          <button class="m3-btn tonal" id="btn-group-sel" ${multiDisabled} title="Group Selected Shapes (Ctrl+G)">Group</button>
          <button class="m3-btn" id="btn-ungroup-sel" ${ungroupDisabled} title="Ungroup Selected Group (Ctrl+Shift+G)">Ungroup</button>
          <button class="m3-btn tonal" data-layout="tb" title="Auto-Arrange Diagram Top-Down">Auto-Layout</button>
        `;
      case 'Insert':
        return `
          <span class="ribbon-group-label">Pages &amp; Stencils</span>
          <button class="m3-btn tonal" id="btn-ribbon-add-page">+ Blank Page</button>
          <button class="m3-btn active" id="btn-ribbon-import-vssx">${this.i18n.importVisioStencil}</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Diagram Structure</span>
          <button class="m3-btn tonal" id="btn-ins-container">Container Box</button>
          <button class="m3-btn tonal" id="btn-ins-swimlanes">Swimlanes</button>
          <button class="m3-btn tonal" id="btn-ins-callout">Callout Note</button>
          <button class="m3-btn" id="btn-ins-image">Image...</button>
          <button class="m3-btn" id="btn-ins-ctrl-handle" ${this.primarySelectedId ? '' : 'disabled'}>Control Handle</button>
          <button class="m3-btn" id="btn-open-link-modal" ${this.primarySelectedId ? '' : 'disabled'}>Hyperlink...</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Engineering &amp; CAD</span>
          <button class="m3-btn" id="btn-ins-room">Room (Live SqFt)</button>
          <button class="m3-btn" id="btn-ins-rack">42U Server Rack</button>
          <button class="m3-btn tonal" id="btn-ins-cad">Import DWG / DXF...</button>
          <button class="m3-btn" id="btn-export-docx">Export Word Spec (.docx)</button>
          <button class="m3-btn" id="btn-export-vsd">Export Legacy .VSD</button>
        `;
      case 'Design':
        return `
          <span class="ribbon-group-label">Page Size &amp; Units</span>
          <select class="cell-input" id="page-preset-select" style="width:210px;height:28px;padding:2px 8px;font-size:11.5px" title="ISO Metric (A0-A5) &amp; US Standard Page Sizes">
            ${PAGE_PRESET_OPTIONS.map(
              (p) => `<option value="${p.id}" ${this.activePagePresetId === p.id ? 'selected' : ''}>${p.label}</option>`
            ).join('')}
          </select>
          <button class="m3-btn ${this.activePagePresetId === 'a4_l' ? 'active' : 'tonal'}" data-page-preset="a4_l">A4</button>
          <button class="m3-btn ${this.activePagePresetId === 'a3_l' ? 'active' : 'tonal'}" data-page-preset="a3_l">A3</button>
          <button class="m3-btn ${this.activePagePresetId === 'letter_l' ? 'active' : 'tonal'}" data-page-preset="letter_l">Letter</button>
          <button class="fmt-btn ${this.metricUnits ? 'active' : ''}" id="btn-unit-mm" title="Metric Units (mm)">mm</button>
          <button class="fmt-btn ${!this.metricUnits ? 'active' : ''}" id="btn-unit-in" title="Imperial Units (in)">in</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Theme</span>
          <select class="cell-input" id="theme-slug-select" style="width:122px;height:28px;padding:2px 8px">
            ${['modern', 'professional', 'vibrant', 'blueprint', 'monochrome', 'hand_drawn']
              .map(
                (t) =>
                  `<option value="${t}" ${this.selectedThemeSlug === t ? 'selected' : ''}>${t.replace('_', ' ')}</option>`
              )
              .join('')}
          </select>
          <select class="cell-input" id="theme-variant-select" style="width:90px;height:28px;padding:2px 8px">
            ${[0, 1, 2, 3]
              .map(
                (v) =>
                  `<option value="${v}" ${this.selectedVariantIx === v ? 'selected' : ''}>Variant ${v + 1}</option>`
              )
              .join('')}
          </select>
          <button class="m3-btn active" id="btn-apply-theme">Apply</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Effects &amp; Layout</span>
          <button class="m3-btn" data-vis-preset="grad_linear" ${selDisabled}>Linear Grad</button>
          <button class="m3-btn" data-vis-preset="shadow_soft" ${selDisabled}>Shadow</button>
          <button class="m3-btn" data-vis-preset="bevel_3d" ${selDisabled}>3D Bevel</button>
          <button class="m3-btn" data-layout="tb">Top-Down</button>
          <button class="m3-btn" data-layout="lr">Left-Right</button>
          <button class="m3-btn" id="btn-iso-title-block">ISO Title Block</button>
        `;
      case 'Data':
        return `
          <span class="ribbon-group-label">External Data &amp; Plan 2 Connectors</span>
          <button class="m3-btn tonal" id="btn-open-data-modal">Data Link &amp; Graphics...</button>
          <button class="m3-btn tonal" id="btn-open-dataviz-modal">Excel Data Visualizer...</button>
          <button class="m3-btn tonal" id="btn-open-cloud-modal">Cloud Discovery (AWS/Azure/GCP)...</button>
          <button class="m3-btn tonal" id="btn-open-org-modal">Org Chart Wizard...</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Reports</span>
          <button class="m3-btn" id="btn-export-pbi">Power BI (.pbi.json)</button>
          <button class="m3-btn" id="btn-export-bom">BOM Report (.xlsx)</button>
        `;
      case 'Process':
        return `
          <span class="ribbon-group-label">Subprocess</span>
          <button class="m3-btn active" id="btn-create-subprocess" ${this.primarySelectedId ? '' : 'disabled'}>Link / Create Subprocess</button>
          <button class="m3-btn tonal" id="btn-export-flow">Power Automate (.flow.json)</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Validate Rules</span>
          <button class="m3-btn" data-validate="bpmn">BPMN 2.0</button>
          <button class="m3-btn" data-validate="uml">UML 2.5</button>
          <button class="m3-btn" data-validate="workflow">SharePoint</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Schedules</span>
          <button class="m3-btn" id="btn-gen-pert">PERT CPM</button>
          <button class="m3-btn" id="btn-gen-gantt">Gantt Chart</button>
          <button class="m3-btn" id="btn-gen-timeline">Timeline</button>
        `;
      case 'Review':
        return `
          <span class="ribbon-group-label">Markup Ink</span>
          <button class="m3-btn ${this.activeTool === 'pen' ? 'active' : ''}" data-tool="pen">Pen Ink</button>
          <button class="m3-btn ${this.activeTool === 'highlighter' ? 'active' : ''}" data-tool="highlighter">Highlighter</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Compare &amp; Review (${snap.comments.length} notes)</span>
          <button class="m3-btn tonal" id="btn-open-compare-modal">Compare Diagrams...</button>
          <button class="m3-btn" id="btn-add-quick-comment">Comments</button>
          <button class="m3-btn" id="btn-open-versions">Version History...</button>
        `;
      case 'View':
        return `
          <span class="ribbon-group-label">Workspace Panes</span>
          <button class="m3-btn ${this.showLeftStencils ? 'active' : ''}" id="btn-view-stencils">Shapes Library</button>
          <button class="m3-btn ${this.showRightInspector ? 'active' : ''}" id="btn-view-inspector">Format Inspector</button>
          <button class="m3-btn ${this.showFormulaBar ? 'active' : ''}" id="btn-toggle-fx-bar">Formula Bar (fx)</button>
          <button class="m3-btn ${this.showShapeSheet ? 'active' : ''}" id="btn-toggle-ss">ShapeSheet (Shift+F5)</button>
          <button class="m3-btn ${this.showMinimap ? 'active' : ''}" id="btn-toggle-minimap">Overview Map</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Canvas Aids</span>
          <button class="m3-btn ${this.showRulers ? 'active' : ''}" id="btn-toggle-rulers">Rulers</button>
          <button class="m3-btn ${this.showGrid ? 'active' : ''}" id="btn-toggle-grid">Dot Grid</button>
          <button class="m3-btn ${this.showSmartGuides ? 'active' : ''}" id="btn-toggle-smart-guides" title="Toggle Dynamic Grid Smart Alignment &amp; Equal-Spacing Guides (Hold Alt to bypass)">Smart Guides</button>
          <button class="m3-btn" id="btn-open-layers">Layers (${snap.layers.length})</button>
          <button class="m3-btn" id="btn-fit-window">Fit Sheet</button>
          <button class="m3-btn" id="btn-zoom-100">100%</button>
          <button class="m3-btn" id="btn-recover-opfs">Recover Journal</button>
        `;
      case 'Developer':
      default:
        return `
          <span class="ribbon-group-label">ShapeSheet &amp; Formula</span>
          <button class="m3-btn ${this.showFormulaBar ? 'active' : ''}" id="btn-dev-fx-bar">Formula Bar (fx)</button>
          <button class="m3-btn ${this.showShapeSheet ? 'active' : ''}" id="btn-dev-ss">ShapeSheet</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Boolean Shape Operations</span>
          <button class="m3-btn" data-bool-op="union" ${multiDisabled}>Union</button>
          <button class="m3-btn" data-bool-op="combine" ${multiDisabled}>Combine</button>
          <button class="m3-btn" data-bool-op="fragment" ${multiDisabled}>Fragment</button>
          <button class="m3-btn" data-bool-op="intersect" ${multiDisabled}>Intersect</button>
          <button class="m3-btn" data-bool-op="subtract" ${multiDisabled}>Subtract</button>
          <button class="m3-btn" data-bool-op="offset" ${selDisabled}>Offset</button>
          <button class="m3-btn ${this.showVertexGrips ? 'active' : ''}" id="btn-toggle-vertices" ${this.primarySelectedId ? '' : 'disabled'}>Edit Vertices</button>
          <div class="ribbon-divider"></div>
          <span class="ribbon-group-label">Studios</span>
          <button class="m3-btn tonal" id="btn-open-vba-modal">VBA Macros...</button>
          <button class="m3-btn tonal" id="btn-open-sql-modal">SQL ⇄ ERD...</button>
        `;
    }
  }

  private renderPortsHtml(snap: EngineSnapshot, ppi: number): string {
    const selectedSet = new Set<number>(this.selectedShapeIds);
    const targetIds = new Set<number>();
    if (this.activeTool === 'connector') {
      for (const s of snap.shapes) {
        if (s.kind !== 'Connector1D') targetIds.add(s.id);
      }
    } else if (this.hoveredShapeId && !selectedSet.has(this.hoveredShapeId)) {
      // Only show perimeter green ports on hovered UNSELECTED shapes in Pointer mode so N/S/E/W resize handles are 100% unobstructed!
      targetIds.add(this.hoveredShapeId);
    }
    const dots: string[] = [];
    for (const sh of snap.shapes) {
      if (sh.kind === 'Connector1D' || !targetIds.has(sh.id)) continue;
      const ports =
        sh.ports && sh.ports.length > 0
          ? sh.ports
          : ([
              [sh.pinX, sh.pinY + sh.height * 0.5],
              [sh.pinX + sh.width * 0.5, sh.pinY],
              [sh.pinX, sh.pinY - sh.height * 0.5],
              [sh.pinX - sh.width * 0.5, sh.pinY],
            ] as Array<[number, number]>);
      const cells = sh.portCells ?? [];
      for (let i = 0; i < ports.length; i++) {
        const [px, py] = ports[i];
        const lx = px * ppi;
        const ty = (snap.pageHeightIn - py) * ppi;
        const portCell = cells[i] ?? `Connections.X${i + 1}`;
        dots.push(
          `<div class="connection-port-dot" data-port-shape="${sh.id}" data-port-cell="${portCell}" data-port-x="${px.toFixed(4)}" data-port-y="${py.toFixed(4)}" style="left:${lx.toFixed(1)}px;top:${ty.toFixed(1)}px" title="Drag from Connection Port (${portCell}) to Connect"></div>`
        );
      }
    }

    // Visio AutoConnect outward directional triangles on primary selected 2D shape (`North` cleanly above `28px` Rotate button!)
    if (this.activeTool === 'pointer' && this.selectedShapeIds.length === 1) {
      const selSh = snap.shapes.find(
        (s) => s.id === this.selectedShapeIds[0] && s.kind !== 'Connector1D' && !s.isSwimlane
      );
      if (selSh) {
        const sideOffIn = 22 / ppi;
        const northOffIn = 50 / ppi;
        const angRad = ((selSh.angleDeg || 0) * Math.PI) / 180;
        const cosA = Math.cos(angRad);
        const sinA = Math.sin(angRad);
        const toPage = (dx: number, dy: number): [number, number] => [
          selSh.pinX + dx * cosA - dy * sinA,
          selSh.pinY + dx * sinA + dy * cosA,
        ];
        const hw = selSh.width * 0.5;
        const hh = selSh.height * 0.5;
        const autoDirs: Array<{
          dir: string;
          label: string;
          arrowPt: [number, number];
          portPt: [number, number];
          rotDeg: number;
        }> = [
          {
            dir: 'north',
            label: 'North',
            arrowPt: toPage(0, hh + northOffIn),
            portPt: toPage(0, hh),
            rotDeg: -(selSh.angleDeg || 0),
          },
          {
            dir: 'east',
            label: 'East',
            arrowPt: toPage(hw + sideOffIn, 0),
            portPt: toPage(hw, 0),
            rotDeg: 90 - (selSh.angleDeg || 0),
          },
          {
            dir: 'south',
            label: 'South',
            arrowPt: toPage(0, -(hh + sideOffIn)),
            portPt: toPage(0, -hh),
            rotDeg: 180 - (selSh.angleDeg || 0),
          },
          {
            dir: 'west',
            label: 'West',
            arrowPt: toPage(-(hw + sideOffIn), 0),
            portPt: toPage(-hw, 0),
            rotDeg: 270 - (selSh.angleDeg || 0),
          },
        ];
        for (const item of autoDirs) {
          const [ax, ay] = item.arrowPt;
          const [portX, portY] = item.portPt;
          dots.push(
            `<div class="autoconnect-arrow" data-autoconnect-dir="${item.dir}" data-port-shape="${selSh.id}" data-port-x="${portX.toFixed(4)}" data-port-y="${portY.toFixed(4)}" style="left:${(ax * ppi).toFixed(1)}px;top:${((snap.pageHeightIn - ay) * ppi).toFixed(1)}px;transform:translate(-50%,-50%) rotate(${item.rotDeg.toFixed(1)}deg)" title="Click to AutoConnect ${item.label} • Drag to Connect"><svg width="13" height="13" viewBox="0 0 14 14" style="pointer-events:none"><polygon points="7,1.4 12.6,11.6 1.4,11.6" fill="currentColor" stroke="#1d4ed8" stroke-width="1.35" stroke-linejoin="round"/></svg></div>`
          );
        }
      }
    }
    return dots.join('');
  }

  private renderSelectionOverlays(snap: EngineSnapshot, ppi: number): string {
    const selected2D = snap.shapes.filter(
      (s) => this.selectedShapeIds.includes(s.id) && s.kind !== 'Connector1D'
    );
    const selected1D = snap.shapes.filter(
      (s) => this.selectedShapeIds.includes(s.id) && s.kind === 'Connector1D'
    );

    let groupFrameHtml = '';
    if (selected2D.length >= 2) {
      let minX = Infinity;
      let maxX = -Infinity;
      let minY = Infinity;
      let maxY = -Infinity;
      for (const sh of selected2D) {
        minX = Math.min(minX, sh.pinX - sh.width * 0.5);
        maxX = Math.max(maxX, sh.pinX + sh.width * 0.5);
        minY = Math.min(minY, sh.pinY - sh.height * 0.5);
        maxY = Math.max(maxY, sh.pinY + sh.height * 0.5);
      }
      if (Number.isFinite(minX)) {
        const padPx = 6;
        const fl = minX * ppi - padPx;
        const ft = (snap.pageHeightIn - maxY) * ppi - padPx;
        const fw = (maxX - minX) * ppi + padPx * 2;
        const fh = (maxY - minY) * ppi + padPx * 2;
        groupFrameHtml = `<div class="multi-selection-bounding-frame" style="left:${fl.toFixed(1)}px;top:${ft.toFixed(1)}px;width:${fw.toFixed(1)}px;height:${fh.toFixed(1)}px"></div>`;
      }
    }

    const boxesHtml = selected2D
      .map((sh, idx) => {
        const leftPx = (sh.pinX - sh.width * 0.5) * ppi;
        const topPx = (snap.pageHeightIn - sh.pinY - sh.height * 0.5) * ppi;
        const wPx = Math.max(14, sh.width * ppi);
        const hPx = Math.max(14, sh.height * ppi);
        const rotCss = sh.angleDeg ? `transform:rotate(${-sh.angleDeg}deg);` : '';

        if (idx > 0) {
          return `<div class="selection-box-overlay multi" style="left:${leftPx}px;top:${topPx}px;width:${wPx}px;height:${hPx}px;${rotCss}"></div>`;
        }

        const ctrlDiamonds = (sh.controlHandles ?? [])
          .map((ch) => {
            const cxPx = ch.page_x_in * ppi;
            const cyPx = (snap.pageHeightIn - ch.page_y_in) * ppi;
            return `<div class="control-diamond-handle" data-ctrl-shape="${sh.id}" data-ctrl-row="${ch.name}" style="left:${cxPx.toFixed(1)}px;top:${cyPx.toFixed(1)}px" title="${ch.prompt || ch.name} (Drag Yellow Control Handle)"></div>`;
          })
          .join('');

        const vertexDots =
          this.showVertexGrips && this.primarySelectedId === sh.id
            ? this.activeShapeVertices
                .map((vx) => {
                  const vxPx = vx.page_x_in * ppi;
                  const vyPx = (snap.pageHeightIn - vx.page_y_in) * ppi;
                  return `<div class="vertex-edit-grip" data-vtx-shape="${sh.id}" data-vtx-geom="${vx.geom_index}" data-vtx-row="${vx.row_ix}" style="left:${vxPx.toFixed(1)}px;top:${vyPx.toFixed(1)}px" title="Geometry${vx.geom_index + 1}.Row_${vx.row_ix} (${vx.kind})"></div>`;
                })
                .join('')
            : '';

        const subPill = sh.subprocessPage
          ? `<button class="subprocess-pill-btn" data-sub-pill="${sh.subprocessPage}" style="left:${(sh.pinX * ppi).toFixed(1)}px;top:${(topPx - 8).toFixed(1)}px">Open ${sh.subprocessPage} ↗</button>`
          : '';

        return `
          <div class="selection-box-overlay" style="left:${leftPx}px;top:${topPx}px;width:${wPx}px;height:${hPx}px;${rotCss}">
            <div class="rotation-stem"></div>
            <div class="rotation-knob" data-rotate-knob="${sh.id}" title="Drag to Rotate (${sh.angleDeg.toFixed(0)}°)"><svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" style="pointer-events:none"><path d="M21 12a9 9 0 1 1-2.64-6.36"/><polyline points="21 3 21 9 15 9"/></svg></div>
            <div class="selection-handle" data-handle="nw" data-handle-shape="${sh.id}" style="left:-6px;top:-6px" title="Resize NW (Hold Shift to lock aspect)"></div>
            <div class="selection-handle" data-handle="n" data-handle-shape="${sh.id}" style="left:calc(50% - 5.5px);top:-6px" title="Resize Top"></div>
            <div class="selection-handle" data-handle="ne" data-handle-shape="${sh.id}" style="right:-6px;top:-6px" title="Resize NE (Hold Shift to lock aspect)"></div>
            <div class="selection-handle" data-handle="e" data-handle-shape="${sh.id}" style="right:-6px;top:calc(50% - 5.5px)" title="Resize Right"></div>
            <div class="selection-handle" data-handle="se" data-handle-shape="${sh.id}" style="right:-6px;bottom:-6px" title="Resize SE (Hold Shift to lock aspect)"></div>
            <div class="selection-handle" data-handle="s" data-handle-shape="${sh.id}" style="left:calc(50% - 5.5px);bottom:-6px" title="Resize Bottom"></div>
            <div class="selection-handle" data-handle="sw" data-handle-shape="${sh.id}" style="left:-6px;bottom:-6px" title="Resize SW (Hold Shift to lock aspect)"></div>
            <div class="selection-handle" data-handle="w" data-handle-shape="${sh.id}" style="left:-6px;top:calc(50% - 5.5px)" title="Resize Left"></div>
          </div>
          ${ctrlDiamonds}
          ${vertexDots}
          ${subPill}
        `;
      })
      .join('');

    const connHtml = selected1D
      .map((sh) => {
        const pts =
          sh.pathPoints && sh.pathPoints.length >= 2
            ? sh.pathPoints
            : [
                [sh.beginX ?? sh.pinX - sh.width * 0.5, sh.beginY ?? sh.pinY] as [number, number],
                [sh.endX ?? sh.pinX + sh.width * 0.5, sh.endY ?? sh.pinY] as [number, number],
              ];
        const polyPts = pts
          .map(([x, y]) => `${(x * ppi).toFixed(1)},${((snap.pageHeightIn - y) * ppi).toFixed(1)}`)
          .join(' ');
        const bx = (sh.beginX ?? pts[0][0]) * ppi;
        const by = (snap.pageHeightIn - (sh.beginY ?? pts[0][1])) * ppi;
        const ex = (sh.endX ?? pts[pts.length - 1][0]) * ppi;
        const ey = (snap.pageHeightIn - (sh.endY ?? pts[pts.length - 1][1])) * ppi;

        const segDiamonds: string[] = [];
        const isOrtho = (sh.routeStyle ?? 16) !== 1 && (sh.routeStyle ?? 16) !== 2;
        if (isOrtho && pts.length >= 4) {
          for (let seg = 1; seg <= pts.length - 3; seg++) {
            const p0 = pts[seg];
            const p1 = pts[seg + 1];
            const segLen = Math.hypot(p1[0] - p0[0], p1[1] - p0[1]);
            if (segLen < 0.12) continue;
            const mx = (p0[0] + p1[0]) * 0.5 * ppi;
            const my = (snap.pageHeightIn - (p0[1] + p1[1]) * 0.5) * ppi;
            const isVerticalSeg = Math.abs(p1[0] - p0[0]) <= Math.abs(p1[1] - p0[1]);
            const axis = isVerticalSeg ? 'x' : 'y';
            segDiamonds.push(
              `<div class="connector-segment-grip" data-seg-shape="${sh.id}" data-seg-index="${seg}" data-seg-axis="${axis}" style="left:${mx.toFixed(1)}px;top:${my.toFixed(1)}px" title="Drag Orthogonal Segment"></div>`
            );
          }
        }

        return `
          <svg class="live-overlay-svg" viewBox="0 0 ${snap.pageWidthIn * ppi} ${snap.pageHeightIn * ppi}">
            <polyline points="${polyPts}" fill="none" stroke="#2563eb" stroke-width="4.5" stroke-opacity="0.28" stroke-linecap="round" stroke-linejoin="round"/>
            <polyline points="${polyPts}" fill="none" stroke="#2563eb" stroke-width="1.8" stroke-dasharray="5,3" stroke-linecap="round"/>
          </svg>
          <div class="connector-endpoint-grip" data-conn-shape="${sh.id}" data-conn-end="begin" style="left:${bx.toFixed(1)}px;top:${by.toFixed(1)}px" title="Drag Begin Endpoint (Glues to Shape or Port)"></div>
          <div class="connector-endpoint-grip" data-conn-shape="${sh.id}" data-conn-end="end" style="left:${ex.toFixed(1)}px;top:${ey.toFixed(1)}px" title="Drag End Endpoint (Glues to Shape or Port)"></div>
          ${segDiamonds.join('')}
        `;
      })
      .join('');

    let inlineEditorHtml = '';
    if (this.editingShapeId !== null) {
      const editSh = snap.shapes.find((s) => s.id === this.editingShapeId);
      if (editSh) {
        const is1D = editSh.kind === 'Connector1D';
        const belowCap = !is1D && this.shapeHasBelowCaption(editSh);
        const wPx = is1D
          ? 180
          : belowCap
            ? Math.max(150, editSh.width * ppi + 32)
            : Math.max(120, editSh.width * ppi);
        const hPx = is1D || belowCap ? 38 : Math.max(40, editSh.height * ppi);
        const anchorXIn =
          typeof editSh.txtX === 'number' && Number.isFinite(editSh.txtX)
            ? editSh.txtX
            : editSh.pinX;
        const anchorYIn =
          typeof editSh.txtY === 'number' && Number.isFinite(editSh.txtY)
            ? editSh.txtY
            : belowCap
              ? editSh.pinY - editSh.height * 0.5 - 0.18
              : editSh.pinY;
        const leftPx = anchorXIn * ppi - wPx * 0.5;
        const centerYPx = (snap.pageHeightIn - anchorYIn) * ppi;
        const topPx = centerYPx - hPx * 0.5;
        const escapedDraft = this.inlineTextDraft
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;');
        const inlineFont = (editSh.fontFamily || this.textFontFamily || 'Inter').replace(/"/g, "'");
        inlineEditorHtml = `<textarea id="inline-shape-editor" class="inline-shape-editor" style="left:${leftPx.toFixed(1)}px;top:${topPx.toFixed(1)}px;width:${wPx.toFixed(1)}px;height:${hPx.toFixed(1)}px;font-family:'${inlineFont}',sans-serif" placeholder="Type shape text...">${escapedDraft}</textarea>`;
      }
    }

    return `<div id="port-overlay-layer" class="live-overlay-svg">${this.renderPortsHtml(snap, ppi)}</div>${groupFrameHtml}${boxesHtml}${connHtml}${inlineEditorHtml}`;
  }

  public render(): void {
    const root = document.getElementById('app');
    if (!root || !this.snapshot) return;

    // Preserve scroll positions across DOM rebuilds
    const prevStencilScroll = (document.querySelector('.stencil-pane') as HTMLElement | null)?.scrollTop ?? 0;
    const prevVpEl = document.querySelector('.canvas-viewport') as HTMLElement | null;
    const prevVpScrollLeft = prevVpEl?.scrollLeft ?? 0;
    const prevVpScrollTop = prevVpEl?.scrollTop ?? 0;
    const prevInspScroll = (document.querySelector('.inspector-pane') as HTMLElement | null)?.scrollTop ?? 0;
    const prevSsScroll = (document.querySelector('.shapesheet-dock') as HTMLElement | null)?.scrollTop ?? 0;

    document.documentElement.dataset.theme = this.uiTheme;
    const snap = this.snapshot;
    const selectedShape =
      this.primarySelectedId !== null
        ? snap.shapes.find((s) => s.id === this.primarySelectedId) ?? null
        : null;

    if (selectedShape) {
      const matchedCell = selectedShape.cells.find((c) => c.name === this.activeFormulaCell);
      if (matchedCell) {
        this.activeFormulaDraft = matchedCell.formula;
      }
      if (selectedShape.fontFamily) {
        this.textFontFamily = selectedShape.fontFamily;
      }
      if (typeof selectedShape.fontSizePt === 'number' && Number.isFinite(selectedShape.fontSizePt)) {
        this.textFontSizePt = Math.max(6, Math.min(96, Math.round(selectedShape.fontSizePt)));
      }
      if (typeof selectedShape.charStyle === 'number') {
        this.textBold = (selectedShape.charStyle & 1) !== 0;
        this.textItalic = (selectedShape.charStyle & 2) !== 0;
        this.textUnderline = (selectedShape.charStyle & 4) !== 0;
        this.textStrikethrough = (selectedShape.charStyle & 8) !== 0;
      }
      if (typeof selectedShape.horizAlign === 'number') {
        this.textHorizAlign = selectedShape.horizAlign;
      }
      if (typeof selectedShape.vertAlign === 'number') {
        this.textVertAlign = selectedShape.vertAlign;
      }
      if (typeof selectedShape.bulletStyle === 'number') {
        this.textBullets = selectedShape.bulletStyle > 0;
      }
      if (selectedShape.textColor && /^#[0-9a-fA-F]{6}$/.test(selectedShape.textColor)) {
        this.textColorHex = selectedShape.textColor.toUpperCase();
      }
      this.textBgColorHex =
        selectedShape.textBgColor && /^#[0-9a-fA-F]{6}$/.test(selectedShape.textBgColor)
          ? selectedShape.textBgColor.toUpperCase()
          : 'none';
      const fillCell = selectedShape.cells.find((c) => c.name === 'FillForegnd');
      const parsedFill = this.parseFormulaOrValueHex(fillCell?.formula, fillCell?.value);
      if (parsedFill) this.shapeFillHex = parsedFill;
      const lineCell = selectedShape.cells.find((c) => c.name === 'LineColor');
      const parsedLine = this.parseFormulaOrValueHex(lineCell?.formula, lineCell?.value);
      if (parsedLine) this.shapeLineHex = parsedLine;
    }

    const catalogKeys = Object.keys(this.stencilCatalog);
    const orderedPacks = [
      ...this.packOrder.filter((p) => catalogKeys.includes(p)),
      ...catalogKeys.filter((p) => !this.packOrder.includes(p)),
    ];
    const totalMasterCount = orderedPacks.reduce(
      (sum, pack) => sum + (this.stencilCatalog[pack]?.length ?? 0),
      0
    );

    const q = this.stencilFilter.trim().toLowerCase();
    const searchResults: Array<{ pack: string; name: string; svg: string }> = [];
    if (q.length > 0) {
      for (const pack of orderedPacks) {
        for (const m of this.stencilCatalog[pack] ?? []) {
          if (m.toLowerCase().includes(q) || pack.toLowerCase().includes(q)) {
            searchResults.push({
              pack,
              name: m,
              svg: this.getStencilSvgPreview(pack, m),
            });
          }
        }
      }
    }

    const cellOptionsSet = new Set<string>([
      'Width',
      'Height',
      'PinX',
      'PinY',
      'Angle',
      'Rounding',
      'FillForegnd',
      'FillBkgnd',
      'LineColor',
      'LineWeight',
      'LinePattern',
      'ShapeRouteStyle',
      'BeginArrow',
      'EndArrow',
      'BeginX',
      'BeginY',
      'EndX',
      'EndY',
      'Prop.Cost',
      'User.Custom',
    ]);
    for (const c of selectedShape?.cells ?? []) {
      cellOptionsSet.add(c.name);
    }

    const ppi = 96;
    const ppiZoom = ppi * (this.zoomPct / 100);
    const sheetPxW = snap.pageWidthIn * ppi;
    const sheetPxH = snap.pageHeightIn * ppi;
    const mainGridClasses = [
      'workspace-main',
      this.showLeftStencils ? '' : 'hide-stencils',
      this.showRightInspector ? '' : 'hide-inspector',
    ]
      .filter(Boolean)
      .join(' ');

    const t = this.i18n;
    const ribbonTabLabels: Array<[string, string]> = [
      ['Home', t.homeTab],
      ['Insert', t.insertTab],
      ['Design', t.designTab],
      ['Data', t.dataTab],
      ['Process', t.processTab],
      ['Review', t.reviewTab],
      ['View', t.viewTab],
      ['Developer', t.developerTab],
    ];

    this.isRebuildingDom = true;
    root.innerHTML = `
      <div class="workstation-shell ${this.isRibbonCollapsed ? 'ribbon-collapsed' : ''}">
        <!-- Row 1: Clean ChromeOS Header + Top-Left Inline Rename + Hover-Switching Menus + Collapsible Visio Ribbon Tabs -->
        <header class="wco-titlebar">
          <div class="brand-lockup no-drag" id="btn-brand-backstage" title="${t.startHeroTitle}">
            <img src="/icons/vexio-icon.svg" alt="Vexio" width="22" height="22" style="display:block;flex-shrink:0" />
            <span class="brand-name">Vexio</span>
          </div>

          ${
            this.isRenamingDocTitle
              ? `<input id="doc-title-inline-input" class="doc-title-inline-input no-drag" value="${this.renameDocDraft.replace(/"/g, '&quot;')}" aria-label="Document Name" />`
              : `<span class="doc-title-pill no-drag" id="doc-title-pill" title="${t.renameHint} (${this.currentFileName})">${this.currentFileName}</span>`
          }

          <div class="top-menubar no-drag" role="menubar">
            <div class="top-menu-wrap" data-top-menu="file">
              <button class="top-menu-trigger file-accent ${this.openHeaderMenu === 'file' ? 'open' : ''}" data-top-trigger="file">${t.fileMenu}</button>
              ${
                this.openHeaderMenu === 'file'
                  ? `<div class="top-menu-dropdown" role="menu">
                      <button class="ctx-item" data-top-cmd="backstage"><span>Start Hub &amp; Templates…</span></button>
                      <div class="ctx-sep"></div>
                      <button class="ctx-item" data-top-cmd="new-a4"><span>${t.newBlankA4}</span></button>
                      <button class="ctx-item" data-top-cmd="new-a3"><span>${t.newBlankA3}</span></button>
                      <button class="ctx-item" data-top-cmd="new-letter"><span>${t.newBlankLetter}</span></button>
                      <div class="ctx-sep"></div>
                      <button class="ctx-item" data-top-cmd="open-disk"><span>${t.openLocalFile}</span><span class="ctx-kbd">Ctrl+O</span></button>
                      <button class="ctx-item" data-top-cmd="import-stencil"><span>${t.importVisioStencil}</span></button>
                      <div class="ctx-sep"></div>
                      <button class="ctx-item" data-top-cmd="save-vsdx"><span>${t.saveFile}</span><span class="ctx-kbd">Ctrl+S</span></button>
                      <button class="ctx-item" data-top-cmd="save-as-vsdx"><span>${t.saveAs}</span><span class="ctx-kbd">Ctrl+Shift+S</span></button>
                    </div>`
                  : ''
              }
            </div>

            <div class="top-menu-wrap" data-top-menu="edit">
              <button class="top-menu-trigger ${this.openHeaderMenu === 'edit' ? 'open' : ''}" data-top-trigger="edit">${t.editMenu}</button>
              ${
                this.openHeaderMenu === 'edit'
                  ? `<div class="top-menu-dropdown" role="menu">
                      <button class="ctx-item" data-top-cmd="undo" ${snap.canUndo ? '' : 'disabled'}><span>Undo</span><span class="ctx-kbd">Ctrl+Z</span></button>
                      <button class="ctx-item" data-top-cmd="redo" ${snap.canRedo ? '' : 'disabled'}><span>Redo</span><span class="ctx-kbd">Ctrl+Y</span></button>
                      <div class="ctx-sep"></div>
                      <button class="ctx-item" data-top-cmd="group" ${this.selectedShapeIds.length >= 2 ? '' : 'disabled'}><span>Group Shapes</span><span class="ctx-kbd">Ctrl+G</span></button>
                      <button class="ctx-item" data-top-cmd="ungroup" ${snap.shapes.some((s) => this.selectedShapeIds.includes(s.id) && s.kind === 'Group') ? '' : 'disabled'}><span>Ungroup Shapes</span><span class="ctx-kbd">Ctrl+Shift+G</span></button>
                      <div class="ctx-sep"></div>
                      <button class="ctx-item" data-top-cmd="rotate-left" ${this.selectedShapeIds.length ? '' : 'disabled'}><span>${t.rotateLeft90}</span><span class="ctx-kbd">Ctrl+L</span></button>
                      <button class="ctx-item" data-top-cmd="rotate-right" ${this.selectedShapeIds.length ? '' : 'disabled'}><span>${t.rotateRight90}</span><span class="ctx-kbd">Ctrl+R</span></button>
                      <button class="ctx-item" data-top-cmd="flip-h" ${this.selectedShapeIds.length ? '' : 'disabled'}><span>${t.flipHoriz}</span></button>
                      <button class="ctx-item" data-top-cmd="flip-v" ${this.selectedShapeIds.length ? '' : 'disabled'}><span>${t.flipVert}</span></button>
                      <div class="ctx-sep"></div>
                      <button class="ctx-item" data-top-cmd="rename-doc"><span>Rename Document…</span></button>
                    </div>`
                  : ''
              }
            </div>

            <div class="top-menu-wrap" data-top-menu="export">
              <button class="top-menu-trigger ${this.openHeaderMenu === 'export' ? 'open' : ''}" data-top-trigger="export">${t.exportMenu}</button>
              ${
                this.openHeaderMenu === 'export'
                  ? `<div class="top-menu-dropdown" role="menu">
                      <button class="ctx-item" data-top-cmd="exp-pdf"><span>Export Vector PDF/A (.pdf)</span></button>
                      <button class="ctx-item" data-top-cmd="exp-svg"><span>Copy / Export Vector SVG (.svg)</span></button>
                      <button class="ctx-item" data-top-cmd="exp-png"><span>Export High-DPI PNG (.png)</span></button>
                      <button class="ctx-item" data-top-cmd="exp-jpg"><span>Export High-DPI JPEG (.jpg)</span></button>
                      <div class="ctx-sep"></div>
                      <button class="ctx-item" data-top-cmd="exp-dxf"><span>Export AutoCAD (.dxf)</span></button>
                      <button class="ctx-item" data-top-cmd="exp-pptx"><span>Export PowerPoint (.pptx)</span></button>
                      <button class="ctx-item" data-top-cmd="exp-docx"><span>Export Word Spec (.docx)</span></button>
                    </div>`
                  : ''
              }
            </div>

            <div class="top-menu-wrap" data-top-menu="help">
              <button class="top-menu-trigger ${this.openHeaderMenu === 'help' ? 'open' : ''}" data-top-trigger="help">${t.helpMenu}</button>
              ${
                this.openHeaderMenu === 'help'
                  ? `<div class="top-menu-dropdown" role="menu">
                      <button class="ctx-item" data-top-cmd="check-updates"><span>${t.checkUpdates}</span></button>
                      <button class="ctx-item" data-top-cmd="toggle-auto-updates"><span>${this.autoCheckUpdates ? '✓ ' : ''}${t.autoCheckUpdates}</span></button>
                      <div class="ctx-sep"></div>
                      <button class="ctx-item" data-top-cmd="open-ss"><span>ShapeSheet Studio</span><span class="ctx-kbd">Shift+F5</span></button>
                    </div>`
                  : ''
              }
            </div>
          </div>

          <div class="qat-group no-drag">
            <button class="m3-btn icon-only" id="btn-open" title="${t.openLocalFile} (Ctrl+O)">${UI_ICONS.open}</button>
            <button class="m3-btn icon-only" id="btn-save" title="${t.saveFile} (Ctrl+S)">${UI_ICONS.save}</button>
            <button class="m3-btn" id="btn-save-as" style="height:26px;padding:0 7px;font-size:11px" title="${t.saveAs} (Ctrl+Shift+S)">${t.saveAs}</button>
            <button class="m3-btn icon-only" id="btn-snapshot-quick" title="Save OPFS Version Snapshot">${UI_ICONS.snapshot}</button>
            <button class="m3-btn icon-only" id="btn-undo" title="Undo (Ctrl+Z)" ${snap.canUndo ? '' : 'disabled'}>${UI_ICONS.undo}</button>
            <button class="m3-btn icon-only" id="btn-redo" title="Redo (Ctrl+Y)" ${snap.canRedo ? '' : 'disabled'}>${UI_ICONS.redo}</button>
          </div>

          <nav class="header-ribbon-tabs no-drag" aria-label="Visio Ribbon Tabs">
            ${ribbonTabLabels
              .map(
                ([tabId, label]) =>
                  `<button class="ribbon-tab ${this.activeRibbonTab === tabId && !this.isRibbonCollapsed ? 'active' : ''}" data-tab="${tabId}" title="Click to switch • Double-click to collapse/expand Ribbon">${label}</button>`
              )
              .join('')}
          </nav>

          <button class="m3-btn icon-only no-drag" id="btn-collapse-ribbon" title="${this.isRibbonCollapsed ? 'Expand Ribbon' : 'Collapse Ribbon'}">${this.isRibbonCollapsed ? '▾' : '▴'}</button>

          <div style="flex:1"></div>

          <select class="cell-input no-drag" id="lang-switcher" style="width:68px;height:26px;padding:1px 6px;font-size:11px;font-weight:700" title="Application Language (EN / DE / IT / FR / ES / GR)">
            ${(['en', 'de', 'it', 'fr', 'es', 'gr'] as const)
              .map(
                (code) =>
                  `<option value="${code}" ${this.lang === code ? 'selected' : ''}>${VEXIO_TRANSLATIONS[code].badge}</option>`
              )
              .join('')}
          </select>

          <div class="peer-avatar-stack no-drag" id="peer-avatar-stack"></div>
          <span class="status-chip ${this.cloudState.online ? '' : 'offline'} no-drag" id="cloud-sync-chip">
            ${this.cloudState.online ? (this.cloudState.user ? 'Cloud Synced' : 'Saved Locally') : 'Offline Ready'}
          </span>
          <button class="m3-btn icon-only ${this.showLeftStencils ? 'active' : ''} no-drag" id="btn-strip-stencils" title="Toggle Shapes & Stencils Library">${UI_ICONS.sidebarLeft}</button>
          <button class="m3-btn icon-only ${this.showRightInspector ? 'active' : ''} no-drag" id="btn-strip-inspector" title="Toggle Format & Page Inspector">${UI_ICONS.sidebarRight}</button>
          <button class="m3-btn icon-only no-drag" id="btn-theme-toggle" title="Switch Light / Dark Appearance">
            ${this.uiTheme === 'light' ? UI_ICONS.moon : UI_ICONS.sun}
          </button>
          <button class="m3-btn tonal no-drag" id="btn-auth">
            ${this.cloudState.user ? 'Sign Out' : 'Sign In'}
          </button>
        </header>

        <!-- Row 2: Classic Collapsible Visio Ribbon Command Bar -->
        <div class="ribbon-bar ${this.isRibbonCollapsed ? 'collapsed' : ''}">
          <div class="ribbon-commands">
            ${this.renderContextualRibbon(snap)}
          </div>
        </div>

        <!-- Row 3: Optional Visio fx Formula Bar (Hidden by Default — Toggle in View or Developer) -->
        <div class="formula-strip ${this.showFormulaBar ? '' : 'hidden'}">
          <span class="formula-shape-tag">
            ${selectedShape ? `Sheet.${selectedShape.id} • ${selectedShape.text || selectedShape.name}` : 'PageSheet'}
          </span>
          <input list="cell-datalist" class="cell-input" id="new-cell-name" value="${this.activeFormulaCell.replace(/"/g, '&quot;')}" placeholder="Cell (e.g. Width)" style="width:134px;height:24px;padding:0 6px;font-size:11.5px" />
          <datalist id="cell-datalist">
            ${Array.from(cellOptionsSet)
              .map((cn) => `<option value="${cn}"></option>`)
              .join('')}
          </datalist>
          <span class="fx-badge" title="ShapeSheet Reactive Formula">fx</span>
          <input class="formula-bar-input" id="new-cell-formula" placeholder="=GUARD(2.5 in)" value="${this.activeFormulaDraft.replace(/"/g, '&quot;')}" />
          <label style="display:inline-flex;align-items:center;gap:4px;font-size:11px;color:var(--m3-text-muted);cursor:pointer" title="Override GUARD() formula with SETF">
            <input type="checkbox" id="force-setf-chk" /> SETF
          </label>
          <button class="m3-btn tonal" id="btn-apply-formula" style="height:24px;padding:0 9px;font-size:11.5px">Apply</button>
          <button class="m3-btn ${this.showShapeSheet ? 'active' : ''}" id="btn-strip-ss" style="height:24px;padding:0 9px;font-size:11.5px" title="Toggle Full ShapeSheet Table (Shift+F5)">
            ShapeSheet ${this.showShapeSheet ? '▾' : '▸'}
          </button>
        </div>

        <!-- Row 4: Main 4-Column Workspace (Stencils | Persistent Drawing Tool Rail | Canvas | Inspector) -->
        <main class="${mainGridClasses}">
          <aside class="stencil-pane" style="${this.showLeftStencils ? '' : 'display:none'}">
            <div class="pane-header">
              <span>Shapes (${orderedPacks.length} Packs · ${totalMasterCount})</span>
              <button class="m3-btn" id="btn-expand-collapse-packs" style="height:20px;padding:0 6px;font-size:10px">
                ${this.expandedStencilPacks.size >= orderedPacks.length ? 'Collapse' : 'Expand All'}
              </button>
            </div>

            <button class="m3-btn tonal" id="btn-import-vssx" style="width:100%;height:28px;font-size:11.5px;justify-content:center" title="${t.importVisioStencil}">
              ${t.importVisioStencil}
            </button>
            <input type="file" id="vssx-file-input" accept=".vssx,.vssm,.vss,.vsdx,.vstx,.vstm,.vsd,.vst,.vdx" style="display:none" />

            <div class="search-box-wrap">
              ${UI_ICONS.search}
              <input class="search-input" id="stencil-search" placeholder="Search ${totalMasterCount} shapes in ${orderedPacks.length} packs..." value="${this.stencilFilter.replace(/"/g, '&quot;')}" />
              ${this.stencilFilter ? '<button class="search-clear-btn" id="btn-clear-stencil-search" title="Clear search">×</button>' : ''}
            </div>

            <div class="stencil-group-pills" role="tablist" aria-label="Stencil domain filter">
              ${(
                [
                  ['all', 'All'],
                  ['microsoft', 'Microsoft'],
                  ['network', 'Servers & Net'],
                  ['basic', 'Basic & Flow'],
                  ['cloud', 'Cloud & Dev'],
                  ['eng', 'Eng & CAD'],
                ] as const
              )
                .map(
                  ([gid, label]) =>
                    `<button class="stencil-group-pill ${this.stencilGroupFilter === gid ? 'active' : ''}" data-stencil-group="${gid}">${label}</button>`
                )
                .join('')}
            </div>

            ${
              q.length > 0
                ? `
                  <div class="pane-header" style="margin-top:2px">
                    <span>Search Results</span>
                    <span>${searchResults.length} matches</span>
                  </div>
                  <div class="stencil-grid">
                    ${
                      searchResults.length === 0
                        ? '<div style="grid-column:span 2;padding:18px 8px;text-align:center;color:var(--m3-text-muted)">No matching shapes found.</div>'
                        : searchResults
                            .map(
                              (item) => `
                            <div class="stencil-item" draggable="true" data-pack="${item.pack}" data-master="${item.name}" title="${item.pack}: ${item.name} — Click or drag onto canvas (Right-click for options)">
                              <div class="stencil-preview-stage">${item.svg}</div>
                              <div class="stencil-title">${item.name}</div>
                              <div class="stencil-pack-sub">${this.shortPackLabel(item.pack)}</div>
                            </div>`
                            )
                            .join('')
                    }
                  </div>
                `
                : `
                  <div class="stencil-accordion-list">
                    ${orderedPacks
                      .filter((pack) => {
                        switch (this.stencilGroupFilter) {
                          case 'microsoft':
                            return pack.startsWith('Microsoft ');
                          case 'network':
                            return (
                              pack === 'Servers (Standard Roles)' ||
                              pack === 'Network & Rack Equipment' ||
                              pack === 'Microsoft Servers & Active Directory'
                            );
                          case 'basic':
                            return (
                              pack === 'Basic Shapes' ||
                              pack === 'Basic Flowchart' ||
                              pack === 'Arrows & Block Arrows' ||
                              pack === 'Callouts, Notes & Containers' ||
                              pack === 'Cross-Functional Swimlanes'
                            );
                          case 'cloud':
                            return (
                              pack === 'Cloud Architecture (AWS / GCP / Azure)' ||
                              pack === 'BPMN 2.0' ||
                              pack === 'UML 2.5' ||
                              pack === "Database Crow's Foot & IDEF1X" ||
                              pack === 'UI & Application Wireframes'
                            );
                          case 'eng':
                            return (
                              pack === 'Organization Chart' ||
                              pack === 'Floor Plan & Architecture' ||
                              pack === 'Electrical & Electronic (IEEE / IEC)' ||
                              pack === 'P&ID Process Engineering' ||
                              pack === 'Project Schedule (Gantt & Timeline)' ||
                              pack === 'Value Stream Mapping (Lean VSM)'
                            );
                          default:
                            return true;
                        }
                      })
                      .map((pack) => {
                        const masters = this.stencilCatalog[pack] ?? [];
                        const isOpen =
                          this.stencilGroupFilter !== 'all' || this.expandedStencilPacks.has(pack);
                        const useCompactScroll =
                          this.stencilGroupFilter === 'all' && this.expandedStencilPacks.size > 1;
                        return `
                          <div class="stencil-accordion ${isOpen ? 'expanded' : ''}">
                            <button class="stencil-acc-header" data-acc-pack="${pack}" aria-expanded="${isOpen ? 'true' : 'false'}" title="Click to ${isOpen ? 'collapse' : 'expand'} ${pack}">
                              <span>${isOpen ? '▾' : '▸'}</span>
                              <span class="stencil-acc-title">${pack}</span>
                              <span class="stencil-acc-badge">${masters.length}</span>
                            </button>
                            ${
                              isOpen
                                ? `
                                  <div class="stencil-acc-body ${useCompactScroll ? 'compact-scroll' : ''}">
                                    <div class="stencil-grid">
                                      ${masters
                                        .map(
                                          (mName) => `
                                          <div class="stencil-item" draggable="true" data-pack="${pack}" data-master="${mName}" title="${pack}: ${mName} — Click or drag onto canvas">
                                            <div class="stencil-preview-stage">${this.getStencilSvgPreview(pack, mName)}</div>
                                            <div class="stencil-title">${mName}</div>
                                          </div>`
                                        )
                                        .join('')}
                                    </div>
                                  </div>
                                `
                                : ''
                            }
                          </div>
                        `;
                      })
                      .join('')}
                  </div>
                `
            }
          </aside>

          ${this.renderDrawingToolRail()}

          <section class="canvas-shell" style="${this.showRulers ? '' : 'grid-template-columns:0px 1fr;grid-template-rows:0px 1fr'}">
            <div class="ruler-corner">${this.metricUnits ? 'mm' : 'in'}</div>
            <div class="ruler-horiz" id="ruler-horiz">${this.renderHorizRulerSvg(snap.pageWidthIn, ppiZoom)}</div>
            <div class="ruler-vert" id="ruler-vert">${this.renderVertRulerSvg(snap.pageHeightIn, ppiZoom)}</div>
            <div class="canvas-viewport" id="canvas-viewport">
              <div
                class="sheet-scale-box"
                style="width:${((sheetPxW * this.zoomPct) / 100).toFixed(1)}px;height:${((sheetPxH * this.zoomPct) / 100).toFixed(1)}px"
              >
                <div
                  id="diagram-host"
                  class="diagram-sheet-host tool-${this.activeTool} ${this.spaceKeyDown ? 'space-panning' : ''} ${this.showGrid ? '' : 'hide-grid'}"
                  style="width:${sheetPxW}px;height:${sheetPxH}px;transform:scale(${this.zoomPct / 100})"
                >
                  <div id="wgpu-canvas-mount" class="wgpu-canvas-mount"></div>
                  <div class="svg-vector-layer">${snap.svgPreview}</div>
                  ${this.renderSelectionOverlays(snap, ppi)}
                  <div id="peer-cursor-layer" class="peer-cursor-layer"></div>
                  <svg id="gesture-overlay-svg" class="live-overlay-svg" viewBox="0 0 ${sheetPxW} ${sheetPxH}"></svg>
                </div>
              </div>
            </div>

            <!-- Floating Bottom-Left Zoom Controls (Pinned to Canvas Shell) -->
            <div class="floating-zoom-bar">
              <button class="m3-btn icon-only" id="btn-zoom-out" title="Zoom Out (Ctrl+Wheel)" style="width:24px;height:24px">−</button>
              <span class="zoom-readout">${this.zoomPct}%</span>
              <button class="m3-btn icon-only" id="btn-zoom-in" title="Zoom In (Ctrl+Wheel)" style="width:24px;height:24px">+</button>
              <button class="m3-btn" id="btn-zoom-reset" title="Fit Page to Window" style="height:24px;padding:0 7px;font-size:11px">Fit</button>
              <button class="m3-btn" id="btn-zoom-100-pill" title="Actual Size (100%)" style="height:24px;padding:0 6px;font-size:11px">100%</button>
              <button class="m3-btn ${this.showMinimap ? 'active' : ''}" id="btn-zoom-map-pill" title="Toggle Overview Minimap" style="height:24px;padding:0 6px;font-size:11px">Map</button>
            </div>

            ${
              this.showMinimap
                ? `<div class="minimap-dock" title="Click or Drag to Pan Viewport">
                    <div class="minimap-header">
                      <span>Overview</span>
                      <span>${snap.shapes.length} shapes</span>
                    </div>
                    <div class="minimap-stage" id="minimap-stage">
                      ${snap.svgPreview}
                      <div class="minimap-viewport-rect" id="minimap-viewport-rect"></div>
                    </div>
                  </div>`
                : ''
            }
          </section>

          <aside class="inspector-pane" style="${this.showRightInspector ? '' : 'display:none'}">
            <div class="insp-segmented">
              <button class="insp-seg-btn ${this.activeInspectorTab === 'props' ? 'active' : ''}" data-insp="props">${selectedShape ? 'Format' : 'Page'}</button>
              <button class="insp-seg-btn ${this.activeInspectorTab === 'layers' ? 'active' : ''}" data-insp="layers">Layers (${snap.layers.length})</button>
              <button class="insp-seg-btn ${this.activeInspectorTab === 'comments' ? 'active' : ''}" data-insp="comments">Notes (${snap.comments.length})</button>
              <button class="insp-seg-btn ${this.activeInspectorTab === 'validate' ? 'active' : ''}" data-insp="validate">Check (${snap.validationIssues.length})</button>
            </div>

            ${this.renderInspectorBody(snap, selectedShape)}
          </aside>
        </main>

        <!-- Row 5: Optional Bottom ShapeSheet Split-View -->
        <section class="shapesheet-dock ${this.showShapeSheet ? '' : 'hidden'}">
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px">
            <span class="pane-header">ShapeSheet Cells — ${selectedShape ? `${selectedShape.name} (Sheet.${selectedShape.id}) • Click any row to edit in fx bar` : 'PageSheet (Select a shape to inspect cells)'}</span>
            <button class="m3-btn" id="btn-close-ss" style="height:22px;padding:0 8px;font-size:11px">Close ✕</button>
          </div>
          <table class="ss-table">
            <thead><tr><th>Cell Name</th><th>Formula</th><th>Evaluated Value</th><th>Protection</th></tr></thead>
            <tbody>
              ${(selectedShape?.cells ?? [])
                .map(
                  (c) =>
                    `<tr data-ss-cell="${c.name.replace(/"/g, '&quot;')}" data-ss-formula="${c.formula.replace(/"/g, '&quot;')}">
                      <td><strong>${c.name}</strong></td>
                      <td>${c.formula}</td>
                      <td>${c.value}</td>
                      <td>${c.guarded ? '<span class="guard-pill">GUARDED</span>' : '<span style="color:var(--m3-text-muted)">Editable</span>'}</td>
                    </tr>`
                )
                .join('')}
            </tbody>
          </table>
        </section>

        <!-- Row 6: Bottom Page Tabs & Status Strip -->
        <footer class="status-bar">
          <div class="page-tabs">
            ${snap.pages
              .map((p) =>
                this.renamingPageId === p.id
                  ? `<input id="page-rename-inline-input" class="page-rename-inline-input" data-rename-page="${p.id}" value="${this.renamePageDraft.replace(/"/g, '&quot;')}" aria-label="${t.renamePage}" />`
                  : `<button class="m3-btn page-tab-btn ${p.id === snap.activePageId ? 'active' : ''}" data-page="${p.id}" title="Click to switch • Double-click or right-click to rename, duplicate, or delete">${p.name}</button>`
              )
              .join('')}
            <button class="m3-btn page-tab-btn tonal" id="btn-add-page" title="Insert New Page">+ Page</button>
          </div>
          <div style="display:flex;align-items:center;gap:14px;flex-shrink:0">
            <span style="color:var(--m3-text-muted);font-size:11px;white-space:nowrap">Tool: <strong style="color:var(--m3-text);text-transform:capitalize">${this.activeTool.replace('_', ' ')}</strong></span>
            <span id="coord-readout" style="font-variant-numeric:tabular-nums;white-space:nowrap">X: ${this.formatLengthFromIn(this.cursorCoordsIn[0])} &nbsp;Y: ${this.formatLengthFromIn(this.cursorCoordsIn[1])}</span>
            <span style="white-space:nowrap">Selected: <strong>${this.selectedShapeIds.length}</strong></span>
            <label style="display:inline-flex;align-items:center;gap:6px">
              <input type="range" id="zoom-slider" min="25" max="400" value="${this.zoomPct}" style="width:88px;accent-color:var(--m3-primary)" />
            </label>
          </div>
        </footer>

        ${this.renderContextMenu(snap)}
        ${this.toastBanner ? `<div id="vexio-toast-banner" class="vexio-toast-banner ${this.toastBanner.level === 'info' ? '' : this.toastBanner.level}">${this.toastBanner.text}</div>` : ''}
        ${this.renderStudioModal()}
      </div>
    `;

    // Restore scroll positions after DOM rebuild
    const nextStencilEl = document.querySelector('.stencil-pane') as HTMLElement | null;
    if (nextStencilEl) nextStencilEl.scrollTop = prevStencilScroll;
    const nextVpEl = document.querySelector('.canvas-viewport') as HTMLElement | null;
    if (nextVpEl) {
      nextVpEl.scrollLeft = prevVpScrollLeft;
      nextVpEl.scrollTop = prevVpScrollTop;
    }
    const nextInspEl = document.querySelector('.inspector-pane') as HTMLElement | null;
    if (nextInspEl) nextInspEl.scrollTop = prevInspScroll;
    const nextSsEl = document.querySelector('.shapesheet-dock') as HTMLElement | null;
    if (nextSsEl) nextSsEl.scrollTop = prevSsScroll;

    // Re-attach persistent OffscreenCanvas DOM node without losing WebGPU context
    const mount = document.getElementById('wgpu-canvas-mount');
    if (mount && !mount.contains(this.wgpuCanvasEl)) {
      mount.appendChild(this.wgpuCanvasEl);
    }
    this.renderPeersOverlayOnly();
    this.renderStatusChipsOnly();
    this.attachDomListeners();
    this.syncViewportRulersAndMinimap();
    this.isRebuildingDom = false;

    if (this.isRenamingDocTitle) {
      const titleInp = document.getElementById('doc-title-inline-input') as HTMLInputElement | null;
      if (titleInp) {
        titleInp.focus({ preventScroll: true });
        titleInp.select();
      }
    } else if (this.renamingPageId !== null) {
      const pageInp = document.getElementById('page-rename-inline-input') as HTMLInputElement | null;
      if (pageInp) {
        pageInp.focus({ preventScroll: true });
        pageInp.select();
      }
    } else if (this.editingShapeId !== null) {
      const ed = document.getElementById('inline-shape-editor') as HTMLTextAreaElement | null;
      if (ed) {
        const shouldSelectAll = this.pendingInlineSelectAll;
        this.pendingInlineSelectAll = false;
        ed.focus({ preventScroll: true });
        if (shouldSelectAll) {
          ed.setSelectionRange(0, ed.value.length);
          requestAnimationFrame(() => {
            if (document.activeElement !== ed && ed.isConnected) {
              ed.focus({ preventScroll: true });
            }
            if (ed.isConnected) {
              ed.setSelectionRange(0, ed.value.length);
            }
          });
        }
      }
    }
  }

  private renderInspectorBody(snap: EngineSnapshot, selectedShape: EngineSnapshot['shapes'][number] | null): string {
    if (this.activeInspectorTab === 'layers') {
      return `
        <div class="insp-card">
          <div class="insp-card-title">Visio &amp; CAD Layer Manager</div>
          ${
            snap.layers.length === 0
              ? '<div style="color:var(--m3-text-muted);font-size:11.5px">Default layer active.</div>'
              : snap.layers
                  .map(
                    (l) => `
                  <div class="list-row-card" style="padding:7px 10px">
                    <strong>${l.name}</strong>
                    <div style="display:flex;gap:10px;font-size:11px;color:var(--m3-text-secondary)">
                      <label style="cursor:pointer"><input type="checkbox" class="layer-vis-chk" data-ix="${l.ix}" ${l.visible ? 'checked' : ''} /> Visible</label>
                      <label style="cursor:pointer"><input type="checkbox" class="layer-lock-chk" data-ix="${l.ix}" ${l.lock ? 'checked' : ''} /> Lock</label>
                    </div>
                  </div>`
                  )
                  .join('')
          }
        </div>
      `;
    }

    if (this.activeInspectorTab === 'comments') {
      return `
        <div class="insp-card">
          <div class="insp-card-title">Review Notes</div>
          <div style="display:flex;gap:6px">
            <input class="cell-input" id="new-comment-input" placeholder="Add note on selected shape..." />
            <button class="m3-btn file-backstage-btn" id="btn-post-comment">Post</button>
          </div>
          ${
            snap.comments.length === 0
              ? '<div style="color:var(--m3-text-muted);font-size:11.5px;padding:6px 0">No review notes on this page yet.</div>'
              : snap.comments
                  .map(
                    (c) => `
                <div class="list-row-card" style="flex-direction:column;align-items:flex-start;gap:3px">
                  <div style="font-weight:600;color:var(--m3-primary);font-size:11px">${c.author} ${c.shapeId ? `• Sheet.${c.shapeId}` : ''}</div>
                  <div>${c.text}</div>
                </div>`
                  )
                  .join('')
          }
        </div>
      `;
    }

    if (this.activeInspectorTab === 'validate') {
      return `
        <div class="insp-card">
          <div class="insp-card-title">Diagram Validation (${snap.validationIssues.length})</div>
          <div style="display:flex;gap:4px;flex-wrap:wrap">
            <button class="m3-btn tonal" data-validate="bpmn" style="height:26px;font-size:11px">Run BPMN 2.0</button>
            <button class="m3-btn tonal" data-validate="uml" style="height:26px;font-size:11px">Run UML 2.5</button>
          </div>
          ${
            snap.validationIssues.length === 0
              ? '<div style="padding:8px 10px;border-radius:7px;background:var(--m3-secondary-container);color:var(--m3-secondary);font-weight:500">✓ 0 structural rule violations found.</div>'
              : snap.validationIssues
                  .map(
                    (iss) => `
                <div style="padding:8px 10px;border-radius:7px;background:var(--m3-error-container);border-left:3px solid var(--m3-error)">
                  <strong>${iss.code}</strong> (${iss.severity}): ${iss.description}
                </div>`
                  )
                  .join('')
          }
        </div>
      `;
    }

    if (!selectedShape) {
      return `
        <div class="insp-card">
          <div class="insp-card-title">Page Setup (${this.formatLengthFromIn(snap.pageWidthIn)} × ${this.formatLengthFromIn(snap.pageHeightIn)})</div>
          <div class="effect-chips-grid">
            <button class="m3-btn ${this.activePagePresetId === 'a4_l' ? 'active' : 'tonal'}" data-page-preset="a4_l" style="height:28px;font-size:11px">ISO A4 Land</button>
            <button class="m3-btn ${this.activePagePresetId === 'a4_p' ? 'active' : 'tonal'}" data-page-preset="a4_p" style="height:28px;font-size:11px">ISO A4 Port</button>
            <button class="m3-btn ${this.activePagePresetId === 'a3_l' ? 'active' : 'tonal'}" data-page-preset="a3_l" style="height:28px;font-size:11px">ISO A3 Land</button>
            <button class="m3-btn ${this.activePagePresetId === 'a5_l' ? 'active' : 'tonal'}" data-page-preset="a5_l" style="height:28px;font-size:11px">ISO A5</button>
            <button class="m3-btn ${this.activePagePresetId === 'letter_l' ? 'active' : 'tonal'}" data-page-preset="letter_l" style="height:28px;font-size:11px">Letter 11×8.5"</button>
            <button class="m3-btn ${this.activePagePresetId === 'tabloid_l' ? 'active' : 'tonal'}" data-page-preset="tabloid_l" style="height:28px;font-size:11px">Tabloid 17×11"</button>
          </div>
          <div style="display:flex;gap:6px;margin-top:6px">
            <button class="m3-btn ${this.metricUnits ? 'active' : 'tonal'}" id="btn-insp-unit-mm" style="flex:1;height:26px;font-size:11px">Metric (mm)</button>
            <button class="m3-btn ${!this.metricUnits ? 'active' : 'tonal'}" id="btn-insp-unit-in" style="flex:1;height:26px;font-size:11px">Imperial (in)</button>
          </div>
        </div>

        <div class="insp-card">
          <div class="insp-card-title">Diagram Theme</div>
          <div class="effect-chips-grid">
            ${[
              ['modern', 'Modern Blue'],
              ['professional', 'Executive Slate'],
              ['vibrant', 'Vibrant Teal'],
              ['blueprint', 'CAD Blueprint'],
              ['monochrome', 'Clean Mono'],
              ['hand_drawn', 'Wireframe Sketch'],
            ]
              .map(
                ([slug, title]) =>
                  `<button class="m3-btn ${this.selectedThemeSlug === slug ? 'active' : 'tonal'}" data-quick-theme="${slug}" style="height:27px;font-size:11px">${title}</button>`
              )
              .join('')}
          </div>
        </div>

        <div class="insp-card">
          <div class="insp-card-title">Canvas View</div>
          <div style="display:flex;flex-direction:column;gap:6px;font-size:12px">
            <label style="display:flex;align-items:center;gap:8px;cursor:pointer"><input type="checkbox" id="chk-insp-grid" ${this.showGrid ? 'checked' : ''} /> Show Dot Grid</label>
            <label style="display:flex;align-items:center;gap:8px;cursor:pointer"><input type="checkbox" id="chk-insp-rulers" ${this.showRulers ? 'checked' : ''} /> Show Rulers (${this.metricUnits ? 'mm' : 'in'})</label>
            <label style="display:flex;align-items:center;gap:8px;cursor:pointer"><input type="checkbox" id="chk-insp-smart-guides" ${this.showSmartGuides ? 'checked' : ''} /> Smart Guides &amp; Equal Spacing</label>
            <label style="display:flex;align-items:center;gap:8px;cursor:pointer"><input type="checkbox" id="chk-insp-minimap" ${this.showMinimap ? 'checked' : ''} /> Show Overview Minimap</label>
            <label style="display:flex;align-items:center;gap:8px;cursor:pointer"><input type="checkbox" id="chk-insp-fx" ${this.showFormulaBar ? 'checked' : ''} /> Show Developer Formula Bar (fx)</label>
          </div>
        </div>

        <div class="insp-card">
          <div class="insp-card-title">Quick Starter Diagrams</div>
          <div class="effect-chips-grid">
            <button class="m3-btn tonal" data-template-id="blank" style="height:27px;font-size:11px">Blank Sheet</button>
            <button class="m3-btn tonal" data-template-id="bpmn_fulfillment" style="height:27px;font-size:11px">BPMN Process</button>
            <button class="m3-btn tonal" data-template-id="cloud_microservices" style="height:27px;font-size:11px">Cloud K8s</button>
            <button class="m3-btn tonal" data-template-id="sql_ecommerce_erd" style="height:27px;font-size:11px">Database ERD</button>
            <button class="m3-btn tonal" data-template-id="executive_org_chart" style="height:27px;font-size:11px">Org Chart</button>
            <button class="m3-btn tonal" data-template-id="datacenter_floorplan" style="height:27px;font-size:11px">Floor Plan</button>
          </div>
        </div>
      `;
    }

    const is1D = selectedShape.kind === 'Connector1D';
    const inspFonts = this.getAvailableOsFontList(selectedShape.fontFamily);
    const activeTextColor = /^#[0-9a-fA-F]{6}$/.test(this.textColorHex) ? this.textColorHex : '#0F172A';
    const activeBgColor = /^#[0-9a-fA-F]{6}$/.test(this.textBgColorHex) ? this.textBgColorHex : '#FEF08A';

    return `
      <div class="insp-card">
        <div class="insp-card-title" style="display:flex;justify-content:space-between;align-items:center">
          <span>Selected (${this.selectedShapeIds.length})</span>
          <span style="font-weight:600;color:var(--m3-primary);text-transform:none">Sheet.${selectedShape.id}</span>
        </div>
        <select class="search-input" id="shape-selector" style="padding:5px 8px">
          ${snap.shapes
            .map(
              (s) =>
                `<option value="${s.id}" ${selectedShape.id === s.id ? 'selected' : ''}>#${s.id} — ${s.text || s.name}</option>`
            )
            .join('')}
        </select>
      </div>

      <div class="insp-card">
        <div class="insp-card-title">Label &amp; Text Formatting</div>
        <input class="cell-input" id="shape-text-input" value="${selectedShape.text.replace(/"/g, '&quot;')}" placeholder="Type shape label (or double-click shape)..." />
        <div style="display:flex;gap:6px;margin-top:4px;align-items:center">
          <select class="cell-input" id="insp-font-family" style="flex:1;height:26px;padding:1px 6px;font-size:11.5px" title="Installed OS Font Family">
            ${inspFonts
              .map(
                (f) => `<option value="${f.replace(/"/g, '&quot;')}" ${this.textFontFamily === f ? 'selected' : ''}>${f}</option>`
              )
              .join('')}
          </select>
          <label class="native-color-chip" title="Text Color (Native OS Color Picker)">
            <span>A</span>
            <input type="color" id="insp-text-color-picker" class="native-color-input" value="${activeTextColor}" />
          </label>
          <label class="native-color-chip" title="Text Highlight (Native OS Color Picker)">
            <span>ab</span>
            <input type="color" id="insp-text-bg-picker" class="native-color-input" value="${activeBgColor}" />
          </label>
          ${
            this.textBgColorHex !== 'none'
              ? `<button class="fmt-btn" id="btn-insp-clear-bg" title="${this.i18n.clearHighlight}">✕</button>`
              : ''
          }
        </div>
        <div style="display:flex;gap:4px;flex-wrap:wrap;margin-top:4px">
          <button class="fmt-btn ${this.textBold ? 'active' : ''}" id="btn-insp-bold" title="Bold (Ctrl+B)"><strong>B</strong></button>
          <button class="fmt-btn ${this.textItalic ? 'active' : ''}" id="btn-insp-italic" title="Italic (Ctrl+I)"><em>I</em></button>
          <button class="fmt-btn ${this.textUnderline ? 'active' : ''}" id="btn-insp-underline" title="Underline (Ctrl+U)"><u>U</u></button>
          <button class="fmt-btn ${this.textHorizAlign === 0 ? 'active' : ''}" data-halign="0" title="Align Left">≡L</button>
          <button class="fmt-btn ${this.textHorizAlign === 1 ? 'active' : ''}" data-halign="1" title="Center">≡C</button>
          <button class="fmt-btn ${this.textHorizAlign === 2 ? 'active' : ''}" data-halign="2" title="Align Right">≡R</button>
          <button class="fmt-btn ${this.textBullets ? 'active' : ''}" id="btn-insp-bullets" title="Bullets">•</button>
        </div>
      </div>

      <div class="insp-card">
        <div class="insp-card-title">Rotate &amp; Flip Stencil</div>
        <div class="effect-chips-grid">
          <button class="m3-btn tonal" data-rot-deg="-90" style="height:27px;font-size:11px">↺ Left 90°</button>
          <button class="m3-btn tonal" data-rot-deg="90" style="height:27px;font-size:11px">↻ Right 90°</button>
          <button class="m3-btn tonal" data-flip-axis="x" style="height:27px;font-size:11px">⇆ Flip Horiz</button>
          <button class="m3-btn tonal" data-flip-axis="y" style="height:27px;font-size:11px">⇅ Flip Vert</button>
        </div>
      </div>

      ${
        is1D
          ? `
            <div class="insp-card">
              <div class="insp-card-title">Line / Connector Routing &amp; Arrows</div>
              <div class="effect-chips-grid">
                <button class="m3-btn ${(selectedShape.routeStyle ?? 16) !== 1 && (selectedShape.routeStyle ?? 16) !== 2 ? 'active' : 'tonal'}" data-conn-route="16" style="height:26px;font-size:11px">Right-Angle</button>
                <button class="m3-btn ${selectedShape.routeStyle === 2 ? 'active' : 'tonal'}" data-conn-route="2" style="height:26px;font-size:11px">Curved</button>
                <button class="m3-btn ${selectedShape.routeStyle === 1 ? 'active' : 'tonal'}" data-conn-route="1" style="height:26px;font-size:11px">Straight</button>
                <button class="m3-btn ${(selectedShape.endArrow ?? 0) > 0 ? 'active' : 'tonal'}" data-conn-arrow="${(selectedShape.endArrow ?? 0) > 0 ? '0' : '4'}" style="height:26px;font-size:11px">${(selectedShape.endArrow ?? 0) > 0 ? 'Arrow: ON' : 'Arrow: OFF'}</button>
              </div>
            </div>
          `
          : `
            <div class="insp-card">
              <div class="insp-card-title" style="display:flex;justify-content:space-between;align-items:center">
                <span>${this.i18n.fillColor}</span>
                <label class="native-color-chip" title="${this.i18n.fillColor} (Native OS Color Picker)">
                  <span>Pick</span>
                  <input type="color" id="insp-fill-color-picker" class="native-color-input" value="${this.shapeFillHex}" />
                </label>
              </div>
              <div class="swatch-row" style="justify-content:space-between">
                ${['255,255,255', '239,246,255', '219,234,254', '220,252,231', '254,249,195', '254,226,226', '243,232,255', '226,232,240']
                  .map(
                    (rgb) =>
                      `<button class="swatch-btn" data-fill-rgb="${rgb}" style="background:rgb(${rgb})" title="Fill RGB(${rgb})"></button>`
                  )
                  .join('')}
              </div>
            </div>
          `
      }

      <div class="insp-card">
        <div class="insp-card-title" style="display:flex;justify-content:space-between;align-items:center">
          <span>Stroke &amp; Line Style</span>
          <label class="native-color-chip" title="${this.i18n.lineColor} (Native OS Color Picker)">
            <span>Pick</span>
            <input type="color" id="insp-line-color-picker" class="native-color-input" value="${this.shapeLineHex}" />
          </label>
        </div>
        <div class="swatch-row" style="justify-content:space-between">
          ${['15,23,42', '37,99,235', '5,150,105', '217,119,6', '220,38,38', '124,58,237', '100,116,139']
            .map(
              (rgb) =>
                `<button class="swatch-btn" data-line-rgb="${rgb}" style="background:rgb(${rgb})" title="Stroke RGB(${rgb})"></button>`
            )
            .join('')}
        </div>
        <div class="effect-chips-grid">
          <button class="m3-btn ${selectedShape.linePattern === 1 ? 'active' : 'tonal'}" data-line-pat="1" style="height:25px;font-size:11px">Solid</button>
          <button class="m3-btn ${selectedShape.linePattern === 2 ? 'active' : 'tonal'}" data-line-pat="2" style="height:25px;font-size:11px">Dashed</button>
          <button class="m3-btn tonal" data-line-wt="1 pt" style="height:25px;font-size:11px">Thin (1pt)</button>
          <button class="m3-btn tonal" data-line-wt="2.25 pt" style="height:25px;font-size:11px">Bold (2.25pt)</button>
        </div>
      </div>

      <div class="insp-card">
        <div class="insp-card-title">Size &amp; Position</div>
        <div class="dim-grid">
          <label class="dim-field" title="PinX (inches)"><span class="dim-prefix">X</span><input class="quick-dim" data-cell="PinX" value="${selectedShape.pinX.toFixed(2)} in" /></label>
          <label class="dim-field" title="PinY (inches)"><span class="dim-prefix">Y</span><input class="quick-dim" data-cell="PinY" value="${selectedShape.pinY.toFixed(2)} in" /></label>
          <label class="dim-field" title="Width (inches)"><span class="dim-prefix">W</span><input class="quick-dim" data-cell="Width" value="${selectedShape.width.toFixed(2)} in" /></label>
          <label class="dim-field" title="Height (inches)"><span class="dim-prefix">H</span><input class="quick-dim" data-cell="Height" value="${selectedShape.height.toFixed(2)} in" /></label>
          <label class="dim-field" style="grid-column:span 2" title="Rotation Angle"><span class="dim-prefix">∠</span><input class="quick-dim" data-cell="Angle" value="${selectedShape.angleDeg.toFixed(1)} deg" /></label>
        </div>
      </div>

      <div class="insp-card">
        <div class="insp-card-title">Effects &amp; Actions</div>
        <div class="effect-chips-grid">
          <button class="m3-btn tonal" data-vis-preset="shadow_soft" style="height:26px;font-size:11px">Drop Shadow</button>
          <button class="m3-btn tonal" data-vis-preset="grad_linear" style="height:26px;font-size:11px">Gradient</button>
          <button class="m3-btn tonal" data-zorder="front" style="height:26px;font-size:11px">Bring Front</button>
          <button class="m3-btn tonal" data-zorder="back" style="height:26px;font-size:11px">Send Back</button>
        </div>
      </div>

      ${
        selectedShape.subprocessPage
          ? `<button class="m3-btn active" id="btn-jump-subprocess" data-sub-page="${selectedShape.subprocessPage}" style="width:100%">Open Subprocess: ${selectedShape.subprocessPage} →</button>`
          : ''
      }
      ${
        selectedShape.screentip
          ? `<div class="insp-card" style="background:var(--m3-secondary-container);border-color:rgba(5,150,105,0.25);color:var(--m3-secondary)"><strong>ScreenTip:</strong> ${selectedShape.screentip}</div>`
          : ''
      }
    `;
  }

  private renderStudioModal(): string {
    if (!this.activeModal) return '';
    const t = this.i18n;

    if (this.activeModal === 'updates') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>${t.checkUpdates}</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div class="list-row-card" style="flex-direction:column;align-items:flex-start;gap:6px">
              <strong>${this.updateStatusText}</strong>
              <span style="font-size:11.5px;color:var(--m3-text-muted)">Installed build: Vexio v0.3.0-alpha • Offline-First Workbox PWA + GitHub Releases channel</span>
            </div>
            <label style="display:flex;align-items:center;gap:8px;font-size:12px;cursor:pointer">
              <input type="checkbox" id="chk-auto-updates" ${this.autoCheckUpdates ? 'checked' : ''} />
              ${t.autoCheckUpdates}
            </label>
            <div style="display:flex;justify-content:flex-end;gap:8px;flex-wrap:wrap">
              <button class="m3-btn tonal" id="btn-recheck-updates">${t.checkUpdates}</button>
              <a class="m3-btn active" href="${this.latestReleaseUrl}" target="_blank" rel="noopener noreferrer" style="text-decoration:none">Open GitHub Releases ↗</a>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'backstage') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card wide">
            <div class="backstage-hero-banner">
              <div style="display:flex;align-items:center;gap:12px">
                <img src="/icons/vexio-icon.svg" alt="Vexio" width="38" height="38" />
                <div>
                  <strong style="font-size:15.5px;display:block">${t.startHeroTitle}</strong>
                  <span style="font-size:11.5px;color:var(--m3-text-secondary)">${t.startHeroSubtitle}</span>
                </div>
              </div>
              <div style="display:flex;gap:8px;flex-wrap:wrap">
                <button class="m3-btn file-backstage-btn" id="btn-close-modal">${t.continueToCanvas} →</button>
              </div>
            </div>

            <div class="pane-header">Create New Diagram or Open Local File Inside Vexio</div>
            <div style="display:flex;gap:8px;flex-wrap:wrap">
              <button class="m3-btn active" data-blank-preset="a4_l">${t.newBlankA4}</button>
              <button class="m3-btn tonal" data-blank-preset="a3_l">${t.newBlankA3}</button>
              <button class="m3-btn tonal" data-blank-preset="letter_l">${t.newBlankLetter}</button>
              <button class="m3-btn tonal" id="btn-backstage-open">${t.openLocalFile}</button>
              <button class="m3-btn tonal" id="btn-backstage-import-vssx">${t.importVisioStencil}</button>
            </div>

            <div class="pane-header" style="margin-top:4px">8 Built-In Enterprise Starter Templates</div>
            <div class="backstage-grid">
              ${STARTER_TEMPLATES.map(
                (tpl) => `
                <div class="template-card" data-template-id="${tpl.id}">
                  <div style="font-size:10px;color:var(--m3-primary);font-weight:700">${tpl.category}</div>
                  <div style="font-weight:700">${tpl.title}</div>
                  <div style="font-size:11px;color:var(--m3-text-muted)">${tpl.desc}</div>
                </div>
              `
              ).join('')}
            </div>

            <div class="pane-header" style="margin-top:4px">Save &amp; Export Formats (100% Offline)</div>
            <div style="display:flex;gap:8px;flex-wrap:wrap">
              <button class="m3-btn active" data-export-ext="vsdx">${t.saveFile}</button>
              <button class="m3-btn tonal" id="btn-backstage-save-as">${t.saveAs}</button>
              <button class="m3-btn tonal" data-export-ext="pdf">Export PDF/A (.pdf)</button>
              <button class="m3-btn tonal" data-export-ext="png">Export PNG (.png)</button>
              <button class="m3-btn tonal" data-export-ext="jpg">Export JPG (.jpg)</button>
              <button class="m3-btn" data-export-ext="svg">Export Vector (.svg)</button>
              <button class="m3-btn" data-export-ext="pptx">PowerPoint (.pptx)</button>
              <button class="m3-btn" data-export-ext="docx">Word Spec (.docx)</button>
              <button class="m3-btn" data-export-ext="dxf">AutoCAD (.dxf)</button>
            </div>

            <div class="pane-header" style="margin-top:4px">Language, Units &amp; Software Updates</div>
            <div style="display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap">
              <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap">
                ${(['en', 'de', 'it', 'fr', 'es', 'gr'] as const)
                  .map(
                    (code) =>
                      `<button class="m3-btn ${this.lang === code ? 'active' : 'tonal'}" data-set-lang="${code}" style="height:26px;font-size:11px">${VEXIO_TRANSLATIONS[code].badge} · ${VEXIO_TRANSLATIONS[code].nativeName}</button>`
                  )
                  .join('')}
              </div>
              <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap">
                <label style="display:flex;align-items:center;gap:6px;font-size:11.5px;cursor:pointer">
                  <input type="checkbox" id="chk-backstage-auto-updates" ${this.autoCheckUpdates ? 'checked' : ''} />
                  ${t.autoCheckUpdates}
                </label>
                <button class="m3-btn tonal" id="btn-backstage-check-updates">${t.checkUpdates}</button>
              </div>
            </div>

            <div class="pane-header" style="margin-top:6px">Immutable Version History Snapshots (OPFS + Firebase Cloud)</div>
            <div style="display:flex;gap:8px">
              <input class="cell-input" id="version-label-input" value="${this.versionLabelDraft}" placeholder="Version label..." />
              <button class="m3-btn active" id="btn-create-version">Save Version Snapshot</button>
            </div>
            <div style="display:flex;flex-direction:column;gap:6px;max-height:130px;overflow-y:auto">
              ${
                this.opfsVersions.length === 0
                  ? '<div style="font-size:12px;color:var(--m3-text-muted)">No snapshots saved for this document yet.</div>'
                  : this.opfsVersions
                      .map(
                        (v) => `
                      <div class="list-row-card">
                        <div>
                          <strong>${v.label}</strong>
                          <span style="color:var(--m3-text-muted);font-size:11px;margin-left:8px">${v.createdAt} • ${(v.byteLength / 1024).toFixed(1)} KB</span>
                        </div>
                        <button class="m3-btn" data-restore-version="${v.versionId}">Restore</button>
                      </div>
                    `
                      )
                      .join('')
              }
            </div>

            <div class="pane-header" style="margin-top:6px">Local OPFS &amp; Synced Cloud Workspaces</div>
            <div style="display:flex;gap:8px">
              <button class="m3-btn" id="btn-new-workspace">+ Create New Workspace Slot</button>
            </div>
            <div style="display:flex;flex-direction:column;gap:6px;max-height:130px;overflow-y:auto">
              ${this.opfsWorkspaces
                .map(
                  (w) => `
                <div class="list-row-card">
                  <div>
                    <strong>[OPFS] ${w.title}</strong>
                    <span style="color:var(--m3-text-muted);font-size:11px;margin-left:8px">${w.docId} • ${w.updatedAt}</span>
                  </div>
                  <button class="m3-btn" data-open-opfs-ws="${w.docId}" data-ws-title="${w.title}">Switch</button>
                </div>`
                )
                .join('')}
              ${this.cloudWorkspaces
                .map(
                  (cw) => `
                <div class="list-row-card">
                  <div>
                    <strong>[Firebase Cloud] ${cw.title}</strong>
                    <span style="color:var(--m3-secondary);font-size:11px;margin-left:8px">${cw.docId} • ${cw.updatedAt}</span>
                  </div>
                  <button class="m3-btn" data-open-cloud-ws="${cw.docId}" data-ws-title="${cw.title}">Pull &amp; Open</button>
                </div>`
                )
                .join('')}
            </div>

            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'dataviz') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card wide">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Visio Plan 2 Two-Way Data Visualizer (Excel / CSV ⇄ Cross-Functional Flowchart)</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div style="font-size:12px;color:var(--m3-text-muted)">
              Bi-directional sync between <code>Process Step ID, Process Step Description, Next Step ID, Connector Label, Shape Type, Function, Owner</code> tables and auto-routed Swimlane diagrams.
            </div>
            <textarea class="studio-textarea" id="dataviz-editor">${this.dataVizDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px;flex-wrap:wrap">
              <button class="m3-btn" id="btn-pick-dataviz-xlsx">Import .xlsx / .csv File...</button>
              <button class="m3-btn" id="btn-export-dataviz-xlsx">Export Page → Data Visualizer (.xlsx)</button>
              <button class="m3-btn active" id="btn-import-dataviz-csv">Build Swimlane Flowchart from Table</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'cloud') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card wide">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Cloud Architecture Auto-Discovery (AWS / Azure / GCP Inventory JSON)</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div style="font-size:12px;color:var(--m3-text-muted)">
              Generates VPC/VNet boundary containers, compute/database/serverless/CDN topology nodes with <code>Prop.*</code> CIDR/provider metadata, and routed TLS/VPC connectors.
            </div>
            <textarea class="studio-textarea" id="cloud-json-editor">${this.cloudJsonDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn active" id="btn-build-cloud-topology">Generate Cloud Topology Page</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'compare') {
      const rep = this.latestDiffReport;
      return `
        <div class="modal-backdrop">
          <div class="modal-card wide">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Diagram Compare — Visual & ShapeSheet Structural Diff</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div style="display:flex;gap:8px;flex-wrap:wrap">
              <button class="m3-btn active" id="btn-compare-latest-opfs">Compare vs Latest OPFS Version Snapshot</button>
              <button class="m3-btn" id="btn-compare-disk-vsdx">Compare vs Local .vsdx File...</button>
            </div>
            ${
              rep
                ? `
                <div class="diff-summary-grid" style="grid-template-columns:repeat(3,1fr)">
                  <div class="diff-stat-pill"><strong style="color:#16a34a">+${rep.added_count}</strong><div>Added Shapes</div></div>
                  <div class="diff-stat-pill"><strong style="color:#dc2626">-${rep.removed_count}</strong><div>Removed Shapes</div></div>
                  <div class="diff-stat-pill"><strong style="color:#f59e0b">${rep.modified_count}</strong><div>Modified / Formula Diffs</div></div>
                </div>
                <div class="diff-svg-box">${rep.diff_overlay_svg}</div>
                <div style="max-height:135px;overflow-y:auto;display:flex;flex-direction:column;gap:4px;font-size:11.5px">
                  ${rep.entries
                    .map(
                      (e) =>
                        `<div class="list-row-card"><span><strong>${e.page_name} • ${e.shape_name} (#${e.shape_id})</strong></span><span style="color:var(--m3-text-muted)">${e.summary}</span></div>`
                    )
                    .join('')}
                </div>
              `
                : '<div style="color:var(--m3-text-muted);font-size:12px">Save a snapshot (+ Snapshot in titlebar) and edit shapes, or pick a baseline .vsdx file to inspect visual red/green/amber overlays and cell-level diffs.</div>'
            }
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'link') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Multi-Target Hyperlink & Hover ScreenTip</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <label class="pane-header">External URL Address</label>
            <input class="cell-input" id="link-addr-input" value="${this.linkAddressDraft}" placeholder="https://..." />
            <label class="pane-header">Internal Page SubAddress (Optional)</label>
            <input class="cell-input" id="link-sub-input" value="${this.linkSubAddressDraft}" placeholder="Page-2" />
            <label class="pane-header">Hover ScreenTip Comment</label>
            <input class="cell-input" id="link-tip-input" value="${this.linkScreentipDraft}" />
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn active" id="btn-save-link-tip">Apply to Selected Shape</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'org') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Organization Chart Wizard (CSV, Microsoft Entra Graph JSON, or LDAP .ldif)</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <div style="font-size:12px;color:var(--m3-text-muted)">
              Paste <code>Name,Title,Manager,Department</code> CSV, Microsoft Graph <code>{"users":[...]}</code> JSON, or RFC 2849 <code>dn:/cn:/title:/manager:</code> LDIF directory export.
            </div>
            <textarea class="studio-textarea" id="org-csv-editor">${this.orgCsvDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn" id="btn-build-org-entra-ldif">Import as Entra JSON / LDIF</button>
              <button class="m3-btn active" id="btn-build-org-chart">Generate Org Chart from CSV</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'vba') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Sandboxed VBA Automation Studio (UserForms, Events, If/Else, Selection)</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <textarea class="studio-textarea" id="vba-editor">${this.vbaDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn active" id="btn-exec-vba">Execute in Bounded Sandbox</button>
            </div>
          </div>
        </div>
      `;
    }

    if (this.activeModal === 'sql') {
      return `
        <div class="modal-backdrop">
          <div class="modal-card">
            <div style="display:flex;justify-content:space-between;align-items:center">
              <strong>Database ERD Reverse & Forward SQL Schema Engineering</strong>
              <button class="m3-btn" id="btn-close-modal">Close</button>
            </div>
            <textarea class="studio-textarea" id="sql-editor">${this.sqlDraft}</textarea>
            ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
            <div style="display:flex;justify-content:flex-end;gap:8px">
              <button class="m3-btn" id="btn-forward-sql">Forward-Engineer Page → SQL DDL</button>
              <button class="m3-btn active" id="btn-reverse-sql">Reverse-Engineer SQL → Crow's Foot ERD</button>
            </div>
          </div>
        </div>
      `;
    }

    return `
      <div class="modal-backdrop">
        <div class="modal-card">
          <div style="display:flex;justify-content:space-between;align-items:center">
            <strong>External Data Recordset Binding (CSV / Native SQLite 3 Binary) & Live Data Graphics</strong>
            <button class="m3-btn" id="btn-close-modal">Close</button>
          </div>
          <div style="font-size:12px;color:var(--m3-text-muted)">
            Matches <code>Label</code> against shape text on the active page, binds <code>Prop.*</code> Shape Data, applies Color-by-Value + Data Bars + Icon Sets + Text Callouts, and inserts a live Page Legend—or bind directly from an offline <code>.sqlite</code> / <code>.db</code> B-tree file!
          </div>
          <textarea class="studio-textarea" id="csv-editor">${this.csvDraft}</textarea>
          ${this.modalOutputMessage ? `<div style="color:var(--m3-secondary);font-size:12px">${this.modalOutputMessage}</div>` : ''}
          <div style="display:flex;justify-content:flex-end;gap:8px">
            <button class="m3-btn" id="btn-bind-sqlite-file">Bind Local SQLite (.sqlite / .db)...</button>
            <button class="m3-btn active" id="btn-apply-csv-dg">Bind CSV & Apply Data Graphics</button>
          </div>
        </div>
      </div>
    `;
  }

  private async saveNewVersionSnapshot(label: string): Promise<void> {
    const { bytes } = await this.callEngine('saveFormat', { ext: 'vsdx' });
    const versionId = `v-${Date.now().toString(36)}`;
    const opfsCopy = new Uint8Array(bytes);
    const vRes = await this.callOpfs(
      'saveVersion',
      { docId: this.docId, versionId, label, vsdxBytes: opfsCopy },
      [opfsCopy.buffer]
    );
    this.opfsVersions = vRes.versions ?? this.opfsVersions;
    if (this.cloudState.user) {
      await this.firebaseSync.backupVersionToCloud(this.docId, versionId, label, bytes);
    }
  }

  private syncViewportRulersAndMinimap(): void {
    const vp = document.getElementById('canvas-viewport');
    const scaleBox = vp?.querySelector('.sheet-scale-box') as HTMLElement | null;
    if (!vp || !scaleBox) return;

    const vpRect = vp.getBoundingClientRect();
    const boxRect = scaleBox.getBoundingClientRect();
    const offsetX = boxRect.left - vpRect.left - 32;
    const offsetY = boxRect.top - vpRect.top - 32;

    const hSvg = document.querySelector('#ruler-horiz svg') as SVGElement | null;
    if (hSvg) {
      hSvg.style.transform = `translateX(${offsetX.toFixed(1)}px)`;
    }
    const vSvg = document.querySelector('#ruler-vert svg') as SVGElement | null;
    if (vSvg) {
      vSvg.style.transform = `translateY(${offsetY.toFixed(1)}px)`;
    }

    const mmRect = document.getElementById('minimap-viewport-rect');
    if (mmRect && boxRect.width > 1 && boxRect.height > 1) {
      const visLeft = Math.max(0, (vpRect.left - boxRect.left) / boxRect.width);
      const visTop = Math.max(0, (vpRect.top - boxRect.top) / boxRect.height);
      const visW = Math.min(1 - visLeft, vpRect.width / boxRect.width);
      const visH = Math.min(1 - visTop, vpRect.height / boxRect.height);
      mmRect.style.inset = 'auto';
      mmRect.style.left = `${(visLeft * 100).toFixed(1)}%`;
      mmRect.style.top = `${(visTop * 100).toFixed(1)}%`;
      mmRect.style.width = `${Math.max(8, visW * 100).toFixed(1)}%`;
      mmRect.style.height = `${Math.max(8, visH * 100).toFixed(1)}%`;
    }
  }

  private async applyFormulaFromStrip(): Promise<void> {
    const shapeId = this.primarySelectedId;
    if (!shapeId) {
      this.showToast('Select a shape first to apply a ShapeSheet formula.', 'warn');
      return;
    }
    const cellName = (document.getElementById('new-cell-name') as HTMLInputElement | null)?.value.trim() || 'Width';
    const formula = (document.getElementById('new-cell-formula') as HTMLInputElement | null)?.value ?? '';
    const forceSetf = Boolean((document.getElementById('force-setf-chk') as HTMLInputElement | null)?.checked);
    this.activeFormulaCell = cellName;
    this.activeFormulaDraft = formula;
    const res = await this.callEngine('setCellFormula', { shapeId, cellName, formula, forceSetf });
    if (res.applied === false) {
      this.showToast(`Cell "${cellName}" is protected by GUARD(). Check SETF to override.`, 'warn');
    } else {
      this.persistMutationSideEffects(res.wal);
    }
    this.snapshot = res.snapshot;
    this.render();
  }

  private attachDomListeners(): void {
    document.getElementById('btn-backstage')?.addEventListener('click', () => this.openBackstageModal());
    document.getElementById('btn-brand-backstage')?.addEventListener('click', () => this.openBackstageModal());
    document.getElementById('btn-open-versions')?.addEventListener('click', () => this.openBackstageModal());

    document.getElementById('btn-snapshot-quick')?.addEventListener('click', async () => {
      await this.saveNewVersionSnapshot(`Checkpoint ${new Date().toLocaleTimeString()}`);
      this.openBackstageModal();
    });

    document.getElementById('btn-open')?.addEventListener('click', () => this.handleOpenDisk());
    document.getElementById('btn-ins-cad')?.addEventListener('click', () => this.handleOpenDisk());
    document.getElementById('btn-save')?.addEventListener('click', () => this.handleSaveInPlace('vsdx'));
    document.getElementById('btn-save-as')?.addEventListener('click', () => this.handleSaveAs('vsdx'));
    document.getElementById('btn-backstage-save-as')?.addEventListener('click', async () => {
      this.activeModal = null;
      this.render();
      await this.handleSaveAs('vsdx');
    });
    document.getElementById('btn-export-jpg')?.addEventListener('click', () => this.handleSaveInPlace('jpg'));
    document.getElementById('btn-export-png')?.addEventListener('click', () => this.handleSaveInPlace('png'));
    document.getElementById('btn-export-pdf')?.addEventListener('click', () => this.handleSaveInPlace('pdf'));
    document.getElementById('btn-export-pptx')?.addEventListener('click', () => this.handleSaveInPlace('pptx'));
    document.getElementById('btn-export-docx')?.addEventListener('click', () => this.handleSaveInPlace('docx'));
    document.getElementById('btn-export-dxf')?.addEventListener('click', () => this.handleSaveInPlace('dxf'));
    document.getElementById('btn-export-vsd')?.addEventListener('click', () => this.handleSaveInPlace('vsd'));
    document.querySelectorAll('[data-export-ext]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const ext = (btn as HTMLElement).dataset.exportExt ?? 'vsdx';
        this.activeModal = null;
        this.render();
        await this.handleSaveInPlace(ext);
      });
    });
    document.getElementById('btn-export-flow')?.addEventListener('click', async () => {
      const { bytes } = await this.callEngine('saveFormat', { ext: 'powerautomate' });
      await saveLocalDiagramInPlace(null, 'Vexio-PowerAutomate-Definition.flow.json', bytes);
      this.showToast('Exported Power Automate workflow JSON', 'info');
    });
    document.getElementById('btn-export-pbi')?.addEventListener('click', async () => {
      const { bytes } = await this.callEngine('saveFormat', { ext: 'powerbi' });
      await saveLocalDiagramInPlace(null, 'Vexio-PowerBI-VisualMap.pbi.json', bytes);
      this.showToast('Exported Power BI visual mapping JSON', 'info');
    });

    document.getElementById('btn-copy-svg')?.addEventListener('click', async () => {
      if (this.snapshot) {
        await copyDiagramVectorToClipboard(this.snapshot.svgPreview, this.snapshot.title);
        this.showToast('Copied vector SVG to clipboard', 'info');
      }
    });

    document.getElementById('btn-undo')?.addEventListener('click', async () => {
      const res = await this.callEngine('undo');
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });
    document.getElementById('btn-redo')?.addEventListener('click', async () => {
      const res = await this.callEngine('redo');
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.getElementById('btn-auth')?.addEventListener('click', async () => {
      try {
        if (this.cloudState.user) {
          await this.firebaseSync.signOutUser();
          this.showToast('Signed out — continuing 100% offline in OPFS', 'info');
        } else {
          await this.firebaseSync.signInGoogleOrEmulator();
          const nextUser = (this.cloudState as CloudSyncState).user;
          if (nextUser) {
            this.showToast(`Signed in as ${nextUser.email ?? 'Cloud User'}`, 'info');
          } else {
            this.showToast('Cloud sign-in cancelled or unavailable — local OPFS mode remains active.', 'warn');
          }
        }
      } catch (err) {
        this.showToast(
          `Cloud sign-in unavailable (${err instanceof Error ? err.message : 'offline'}) — local OPFS mode active.`,
          'warn'
        );
      }
    });

    // Ribbon Tabs, Collapse Pin, Hover-Switching Top Menus, Inline Rename, Language & Rich Text
    document.querySelectorAll('.ribbon-tab').forEach((btn) => {
      btn.addEventListener('click', () => {
        const nextTab = (btn as HTMLElement).dataset.tab ?? 'Home';
        this.activeRibbonTab = nextTab;
        this.isRibbonCollapsed = false;
        this.render();
      });
      btn.addEventListener('dblclick', (ev) => {
        ev.preventDefault();
        this.isRibbonCollapsed = !this.isRibbonCollapsed;
        this.render();
      });
    });

    document.getElementById('btn-collapse-ribbon')?.addEventListener('click', () => {
      this.isRibbonCollapsed = !this.isRibbonCollapsed;
      this.render();
    });

    // Chrome-style Hover-to-Switch across open sibling top menus (File | Edit | Export | Help)
    document.querySelectorAll('[data-top-menu]').forEach((wrap) => {
      const menuId = (wrap as HTMLElement).dataset.topMenu as 'file' | 'edit' | 'export' | 'help';
      wrap.addEventListener('pointerenter', () => {
        if (this.openHeaderMenu !== null && this.openHeaderMenu !== menuId) {
          this.openHeaderMenu = menuId;
          this.render();
        }
      });
    });
    document.querySelectorAll('[data-top-trigger]').forEach((trig) => {
      trig.addEventListener('click', (ev) => {
        ev.stopPropagation();
        const menuId = (trig as HTMLElement).dataset.topTrigger as 'file' | 'edit' | 'export' | 'help';
        this.openHeaderMenu = this.openHeaderMenu === menuId ? null : menuId;
        this.render();
      });
    });
    document.querySelectorAll('[data-top-cmd]').forEach((item) => {
      item.addEventListener('click', async (ev) => {
        ev.stopPropagation();
        const cmd = (item as HTMLElement).dataset.topCmd ?? '';
        this.openHeaderMenu = null;
        switch (cmd) {
          case 'backstage':
            await this.openBackstageModal();
            break;
          case 'new-a4':
            await this.applyPagePresetById('a4_l', true);
            break;
          case 'new-a3':
            await this.applyPagePresetById('a3_l', true);
            break;
          case 'new-letter':
            await this.applyPagePresetById('letter_l', true);
            break;
          case 'open-disk':
            await this.handleOpenDisk();
            break;
          case 'import-stencil':
            this.render();
            (document.getElementById('vssx-file-input') as HTMLInputElement | null)?.click();
            break;
          case 'save-vsdx':
            this.render();
            await this.handleSaveInPlace('vsdx');
            break;
          case 'save-as-vsdx':
            this.render();
            await this.handleSaveAs('vsdx');
            break;
          case 'undo': {
            const res = await this.callEngine('undo');
            this.snapshot = res.snapshot;
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'redo': {
            const res = await this.callEngine('redo');
            this.snapshot = res.snapshot;
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'group':
            await this.groupSelectedShapes();
            break;
          case 'ungroup':
            await this.ungroupSelectedShapes();
            break;
          case 'rotate-left':
            await this.rotateOrFlipSelected(-90);
            break;
          case 'rotate-right':
            await this.rotateOrFlipSelected(90);
            break;
          case 'flip-h':
            await this.rotateOrFlipSelected(0, true, false);
            break;
          case 'flip-v':
            await this.rotateOrFlipSelected(0, false, true);
            break;
          case 'rename-doc':
            this.isRenamingDocTitle = true;
            this.renameDocDraft = this.currentFileName;
            this.render();
            break;
          case 'exp-pdf':
            this.render();
            await this.handleSaveInPlace('pdf');
            break;
          case 'exp-svg':
            if (this.snapshot) {
              await copyDiagramVectorToClipboard(this.snapshot.svgPreview, this.snapshot.title).catch(() => {});
            }
            this.render();
            await this.handleSaveInPlace('svg');
            break;
          case 'exp-png':
            this.render();
            await this.handleSaveInPlace('png');
            break;
          case 'exp-jpg':
            this.render();
            await this.handleSaveInPlace('jpg');
            break;
          case 'exp-dxf':
            this.render();
            await this.handleSaveInPlace('dxf');
            break;
          case 'exp-pptx':
            this.render();
            await this.handleSaveInPlace('pptx');
            break;
          case 'exp-docx':
            this.render();
            await this.handleSaveInPlace('docx');
            break;
          case 'check-updates':
            await this.checkForUpdates(true);
            break;
          case 'toggle-auto-updates':
            this.autoCheckUpdates = !this.autoCheckUpdates;
            try {
              localStorage.setItem('vexio.autoUpdate', String(this.autoCheckUpdates));
            } catch {
              // Ignore
            }
            this.render();
            break;
          case 'open-ss':
            this.showShapeSheet = !this.showShapeSheet;
            this.render();
            break;
          default:
            this.render();
            break;
        }
      });
    });

    // Double-Click Top-Left File Name to Select 100% & Rename Inline
    document.getElementById('doc-title-pill')?.addEventListener('dblclick', (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      this.isRenamingDocTitle = true;
      this.renameDocDraft = this.currentFileName;
      this.render();
    });
    const docTitleInput = document.getElementById('doc-title-inline-input') as HTMLInputElement | null;
    if (docTitleInput) {
      let titleHandled = false;
      docTitleInput.addEventListener('input', () => {
        this.renameDocDraft = docTitleInput.value;
      });
      docTitleInput.addEventListener('keydown', (ev) => {
        if (ev.key === 'Enter') {
          ev.preventDefault();
          ev.stopPropagation();
          if (!titleHandled) {
            titleHandled = true;
            void this.commitDocTitleRename(docTitleInput.value);
          }
        } else if (ev.key === 'Escape') {
          ev.preventDefault();
          ev.stopPropagation();
          titleHandled = true;
          this.isRenamingDocTitle = false;
          this.render();
        }
      });
      docTitleInput.addEventListener('blur', () => {
        if (this.isRebuildingDom || !docTitleInput.isConnected) return;
        if (!titleHandled) {
          titleHandled = true;
          void this.commitDocTitleRename(docTitleInput.value);
        }
      });
    }

    // 6-Language Switcher (EN, DE, IT, FR, ES, GR)
    const switchLang = (rawCode: string) => {
      const code = normalizeLangCode(rawCode);
      this.lang = code;
      document.documentElement.lang = code === 'gr' ? 'el' : code;
      try {
        localStorage.setItem('vexio.lang', code);
      } catch {
        // Ignore
      }
      this.render();
    };
    document.getElementById('lang-switcher')?.addEventListener('change', (e) => {
      switchLang((e.target as HTMLSelectElement).value);
    });
    document.querySelectorAll('[data-set-lang]').forEach((btn) => {
      btn.addEventListener('click', () => {
        switchLang((btn as HTMLElement).dataset.setLang ?? 'en');
      });
    });

    // Rich Text & Native OS Font / Color Picker Listeners
    document.getElementById('btn-load-os-fonts')?.addEventListener('click', async () => {
      const fonts = await enumerateChromeOsSystemFonts();
      this.systemFonts = fonts;
      this.showToast(`Loaded ${fonts.length} installed OS font families`, 'info');
      this.render();
    });
    document.getElementById('ribbon-font-family')?.addEventListener('change', (e) => {
      void this.applyRichTextPatch({ fontFamily: (e.target as HTMLSelectElement).value });
    });
    document.getElementById('insp-font-family')?.addEventListener('change', (e) => {
      void this.applyRichTextPatch({ fontFamily: (e.target as HTMLSelectElement).value });
    });
    document.getElementById('ribbon-font-size')?.addEventListener('change', (e) => {
      void this.applyRichTextPatch({ fontSizePt: Number((e.target as HTMLSelectElement).value) });
    });
    document.getElementById('btn-font-grow')?.addEventListener('click', () => {
      void this.applyRichTextPatch({ fontSizePt: Math.min(96, this.textFontSizePt + 2) });
    });
    document.getElementById('btn-font-shrink')?.addEventListener('click', () => {
      void this.applyRichTextPatch({ fontSizePt: Math.max(6, this.textFontSizePt - 2) });
    });
    const toggleBold = () => void this.applyRichTextPatch({ bold: !this.textBold });
    const toggleItalic = () => void this.applyRichTextPatch({ italic: !this.textItalic });
    const toggleUnderline = () => void this.applyRichTextPatch({ underline: !this.textUnderline });
    const toggleBullets = () => void this.applyRichTextPatch({ bullet: this.textBullets ? 0 : 1 });
    document.getElementById('btn-fmt-bold')?.addEventListener('click', toggleBold);
    document.getElementById('btn-insp-bold')?.addEventListener('click', toggleBold);
    document.getElementById('btn-fmt-italic')?.addEventListener('click', toggleItalic);
    document.getElementById('btn-insp-italic')?.addEventListener('click', toggleItalic);
    document.getElementById('btn-fmt-underline')?.addEventListener('click', toggleUnderline);
    document.getElementById('btn-insp-underline')?.addEventListener('click', toggleUnderline);
    document.getElementById('btn-fmt-strike')?.addEventListener('click', () => {
      void this.applyRichTextPatch({ strikethrough: !this.textStrikethrough });
    });
    document.getElementById('btn-fmt-bullets')?.addEventListener('click', toggleBullets);
    document.getElementById('btn-insp-bullets')?.addEventListener('click', toggleBullets);
    document.getElementById('btn-apply-text-bg')?.addEventListener('click', () => {
      const nextBg = this.textBgColorHex === 'none' ? '#FEF08A' : 'none';
      void this.applyRichTextPatch({ bgColorHex: nextBg });
    });
    const bindTextColorInput = (id: string) => {
      const el = document.getElementById(id);
      if (!el) return;
      const onPick = (e: Event) => {
        void this.applyRichTextPatch({ colorHex: (e.target as HTMLInputElement).value.toUpperCase() });
      };
      el.addEventListener('input', onPick);
      el.addEventListener('change', onPick);
    };
    const bindTextBgInput = (id: string) => {
      const el = document.getElementById(id);
      if (!el) return;
      const onPick = (e: Event) => {
        void this.applyRichTextPatch({ bgColorHex: (e.target as HTMLInputElement).value.toUpperCase() });
      };
      el.addEventListener('input', onPick);
      el.addEventListener('change', onPick);
    };
    bindTextColorInput('ribbon-text-color-picker');
    bindTextColorInput('insp-text-color-picker');
    bindTextBgInput('ribbon-text-bg-picker');
    bindTextBgInput('insp-text-bg-picker');
    document.getElementById('btn-clear-text-bg')?.addEventListener('click', () => {
      void this.applyRichTextPatch({ bgColorHex: 'none' });
    });
    document.getElementById('btn-insp-clear-bg')?.addEventListener('click', () => {
      void this.applyRichTextPatch({ bgColorHex: 'none' });
    });
    const bindFillColorInput = (id: string) => {
      const el = document.getElementById(id);
      if (!el) return;
      const onPick = (e: Event) => {
        void this.applyShapeFillHex((e.target as HTMLInputElement).value);
      };
      el.addEventListener('input', onPick);
      el.addEventListener('change', onPick);
    };
    const bindLineColorInput = (id: string) => {
      const el = document.getElementById(id);
      if (!el) return;
      const onPick = (e: Event) => {
        void this.applyShapeLineHex((e.target as HTMLInputElement).value);
      };
      el.addEventListener('input', onPick);
      el.addEventListener('change', onPick);
    };
    bindFillColorInput('ribbon-fill-color-picker');
    bindFillColorInput('insp-fill-color-picker');
    bindLineColorInput('ribbon-line-color-picker');
    bindLineColorInput('insp-line-color-picker');
    document.querySelectorAll('[data-halign]').forEach((btn) => {
      btn.addEventListener('click', () => {
        void this.applyRichTextPatch({ horizAlign: Number((btn as HTMLElement).dataset.halign ?? 1) });
      });
    });
    document.querySelectorAll('[data-valign]').forEach((btn) => {
      btn.addEventListener('click', () => {
        void this.applyRichTextPatch({ vertAlign: Number((btn as HTMLElement).dataset.valign ?? 1) });
      });
    });

    // Rotate & Flip All Stencils / Shapes
    document.querySelectorAll('[data-rot-deg]').forEach((btn) => {
      btn.addEventListener('click', () => {
        void this.rotateOrFlipSelected(Number((btn as HTMLElement).dataset.rotDeg ?? 90));
      });
    });
    document.querySelectorAll('[data-flip-axis]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const axis = (btn as HTMLElement).dataset.flipAxis;
        void this.rotateOrFlipSelected(0, axis === 'x', axis === 'y');
      });
    });

    // ISO Metric & US Page Presets + Unit Switcher
    document.getElementById('page-preset-select')?.addEventListener('change', (e) => {
      void this.applyPagePresetById((e.target as HTMLSelectElement).value, false);
    });
    document.querySelectorAll('[data-page-preset]').forEach((btn) => {
      btn.addEventListener('click', () => {
        void this.applyPagePresetById((btn as HTMLElement).dataset.pagePreset ?? 'a4_l', false);
      });
    });
    document.querySelectorAll('[data-blank-preset]').forEach((btn) => {
      btn.addEventListener('click', () => {
        void this.applyPagePresetById((btn as HTMLElement).dataset.blankPreset ?? 'a4_l', true);
      });
    });
    const setUnitSystem = async (metric: boolean) => {
      this.metricUnits = metric;
      try {
        localStorage.setItem('vexio.metric', String(metric));
      } catch {
        // Ignore
      }
      if (this.snapshot) {
        await this.callEngine('setPageSetupWithUnit', {
          widthIn: this.snapshot.pageWidthIn,
          heightIn: this.snapshot.pageHeightIn,
          unitSlug: metric ? 'mm' : 'in',
        });
      }
      this.render();
    };
    document.getElementById('btn-unit-mm')?.addEventListener('click', () => void setUnitSystem(true));
    document.getElementById('btn-insp-unit-mm')?.addEventListener('click', () => void setUnitSystem(true));
    document.getElementById('btn-unit-in')?.addEventListener('click', () => void setUnitSystem(false));
    document.getElementById('btn-insp-unit-in')?.addEventListener('click', () => void setUnitSystem(false));

    // Backstage & Ribbon Quick Actions + Software Updates
    document.getElementById('btn-backstage-open')?.addEventListener('click', () => void this.handleOpenDisk());
    document.getElementById('btn-ribbon-import-vssx')?.addEventListener('click', () => {
      (document.getElementById('vssx-file-input') as HTMLInputElement | null)?.click();
    });
    document.getElementById('btn-backstage-import-vssx')?.addEventListener('click', () => {
      this.activeModal = null;
      this.render();
      (document.getElementById('vssx-file-input') as HTMLInputElement | null)?.click();
    });
    document.getElementById('btn-ribbon-add-page')?.addEventListener('click', async () => {
      const nextNum = (this.snapshot?.pages.length ?? 1) + 1;
      const res = await this.callEngine('addPage', { name: `Page-${nextNum}` });
      this.snapshot = res.snapshot;
      this.selectedShapeIds = [];
      this.persistMutationSideEffects(res.wal);
      this.render();
    });
    const bindAutoUpdateChk = (id: string) => {
      document.getElementById(id)?.addEventListener('change', (e) => {
        this.autoCheckUpdates = (e.target as HTMLInputElement).checked;
        try {
          localStorage.setItem('vexio.autoUpdate', String(this.autoCheckUpdates));
        } catch {
          // Ignore
        }
      });
    };
    bindAutoUpdateChk('chk-auto-updates');
    bindAutoUpdateChk('chk-backstage-auto-updates');
    document.getElementById('btn-recheck-updates')?.addEventListener('click', () => void this.checkForUpdates(true));
    document.getElementById('btn-backstage-check-updates')?.addEventListener('click', () => void this.checkForUpdates(true));

    document.querySelectorAll('[data-tool]').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeTool = ((btn as HTMLElement).dataset.tool as ActiveTool) ?? 'pointer';
        this.editingShapeId = null;
        this.render();
      });
    });

    document.getElementById('btn-copy-sel')?.addEventListener('click', async () => {
      await this.copySelectedShapes();
    });

    document.getElementById('btn-paste-sel')?.addEventListener('click', async () => {
      await this.pasteClipboardAt();
    });

    document.getElementById('btn-duplicate-sel')?.addEventListener('click', async () => {
      if (this.selectedShapeIds.length === 0) return;
      const res = await this.callEngine('duplicateShapes', {
        shapeIds: this.selectedShapeIds,
        dxIn: 0.35,
        dyIn: -0.35,
      });
      this.selectedShapeIds = res.createdIds ?? this.selectedShapeIds;
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.querySelectorAll('[data-zorder]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const mode = (btn as HTMLElement).dataset.zorder ?? 'front';
        const res = await this.callEngine('reorderShapes', {
          shapeIds: this.selectedShapeIds,
          mode,
        });
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects(res.wal);
        this.render();
      });
    });

    document.querySelectorAll('[data-align]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const mode = (btn as HTMLElement).dataset.align ?? 'left';
        const isDist = mode === 'distribute_h' || mode === 'distribute_v';
        if (this.selectedShapeIds.length < (isDist ? 3 : 2)) return;
        const res = isDist
          ? await this.callEngine('distributeShapes', {
              shapeIds: this.selectedShapeIds,
              axis: mode === 'distribute_v' ? 'vertical' : 'horizontal',
            })
          : await this.callEngine('alignShapes', {
              shapeIds: this.selectedShapeIds,
              mode,
            });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.getElementById('btn-group-sel')?.addEventListener('click', () => {
      void this.groupSelectedShapes();
    });
    document.getElementById('btn-ungroup-sel')?.addEventListener('click', () => {
      void this.ungroupSelectedShapes();
    });

    document.querySelectorAll('[data-bool-op]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const op = (btn as HTMLElement).dataset.boolOp ?? 'union';
        if (op !== 'offset' && this.selectedShapeIds.length < 2) return;
        if (op === 'offset' && this.selectedShapeIds.length < 1) return;
        const res = await this.callEngine('applyBooleanOp', {
          shapeIds: this.selectedShapeIds,
          op,
          offsetIn: 0.18,
        });
        this.selectedShapeIds = res.createdIds ?? [];
        if (this.showVertexGrips && this.primarySelectedId) {
          await this.refreshActiveVertices(this.primarySelectedId);
        }
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.getElementById('btn-toggle-vertices')?.addEventListener('click', async () => {
      this.showVertexGrips = !this.showVertexGrips;
      if (this.showVertexGrips && this.primarySelectedId) {
        await this.refreshActiveVertices(this.primarySelectedId);
      } else {
        this.activeShapeVertices = [];
      }
      this.render();
    });

    document.querySelectorAll('[data-insp]').forEach((btn) => {
      btn.addEventListener('click', () => {
        this.activeInspectorTab = ((btn as HTMLElement).dataset.insp as InspectorTab) ?? 'props';
        this.render();
      });
    });

    document.getElementById('btn-delete-sel')?.addEventListener('click', async () => {
      if (this.selectedShapeIds.length === 0) return;
      const res = await this.callEngine('deleteShape', { shapeIds: this.selectedShapeIds });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      this.setSingleSelection(null);
      this.render();
    });

    document.querySelectorAll('[data-fill-rgb]').forEach((sw) => {
      sw.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const rgb = (sw as HTMLElement).dataset.fillRgb;
        let lastRes: any = null;
        let blocked = false;
        for (const sid of this.selectedShapeIds) {
          lastRes = await this.callEngine('setCellFormula', {
            shapeId: sid,
            cellName: 'FillForegnd',
            formula: `=RGB(${rgb})`,
            forceSetf: false,
          });
          if (lastRes?.applied === false) blocked = true;
        }
        if (blocked) {
          this.showToast('FillForegnd is protected by GUARD() on one or more selected shapes.', 'warn');
        }
        if (lastRes) {
          this.persistMutationSideEffects(lastRes.wal);
          this.snapshot = lastRes.snapshot;
          this.render();
        }
      });
    });

    document.querySelectorAll('[data-line-rgb]').forEach((sw) => {
      sw.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const rgb = (sw as HTMLElement).dataset.lineRgb;
        const res = await this.callEngine('batchSetCells', {
          shapeIds: this.selectedShapeIds,
          entries: [{ cell: 'LineColor', formula: `=RGB(${rgb})` }],
          forceSetf: false,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.querySelectorAll('[data-line-pat]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const pat = (btn as HTMLElement).dataset.linePat ?? '1';
        const res = await this.callEngine('batchSetCells', {
          shapeIds: this.selectedShapeIds,
          entries: [{ cell: 'LinePattern', formula: pat }],
          forceSetf: false,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.querySelectorAll('[data-line-wt]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const wt = (btn as HTMLElement).dataset.lineWt ?? '1.25 pt';
        const res = await this.callEngine('batchSetCells', {
          shapeIds: this.selectedShapeIds,
          entries: [{ cell: 'LineWeight', formula: wt }],
          forceSetf: false,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.querySelectorAll('[data-conn-route]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const route = (btn as HTMLElement).dataset.connRoute ?? '16';
        const res = await this.callEngine('batchSetCells', {
          shapeIds: this.selectedShapeIds,
          entries: [{ cell: 'ShapeRouteStyle', formula: route }],
          forceSetf: true,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.querySelectorAll('[data-conn-arrow]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const arrow = (btn as HTMLElement).dataset.connArrow ?? '4';
        const res = await this.callEngine('batchSetCells', {
          shapeIds: this.selectedShapeIds,
          entries: [{ cell: 'EndArrow', formula: arrow }],
          forceSetf: true,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      });
    });

    document.getElementById('theme-slug-select')?.addEventListener('change', (e) => {
      this.selectedThemeSlug = (e.target as HTMLSelectElement).value;
    });
    document.getElementById('theme-variant-select')?.addEventListener('change', (e) => {
      this.selectedVariantIx = Number((e.target as HTMLSelectElement).value);
    });
    document.getElementById('btn-apply-theme')?.addEventListener('click', async () => {
      const res = await this.callEngine('applyThemeVariant', {
        themeSlug: this.selectedThemeSlug,
        variantIndex: this.selectedVariantIx,
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });
    document.querySelectorAll('[data-quick-theme]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const slug = (btn as HTMLElement).dataset.quickTheme ?? 'modern';
        this.selectedThemeSlug = slug;
        const res = await this.callEngine('applyThemeVariant', {
          themeSlug: slug,
          variantIndex: this.selectedVariantIx,
        });
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects(res.wal);
        this.render();
      });
    });

    document.querySelectorAll('[data-vis-preset]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        if (this.selectedShapeIds.length === 0) return;
        const preset = (btn as HTMLElement).dataset.visPreset ?? 'shadow';
        const res = await this.callEngine('applyShapeVisualPreset', {
          shapeIds: this.selectedShapeIds,
          preset,
        });
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects(res.wal);
        this.render();
      });
    });

    document.getElementById('btn-iso-title-block')?.addEventListener('click', async () => {
      const res = await this.callEngine('applyTitleBlockBorder', {
        projectCode: 'VEX-ARCH-2026',
        author: this.cloudState.user?.email ?? 'Principal Systems Architect',
        revision: 'Rev 2.0',
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.getElementById('btn-ins-image')?.addEventListener('click', async () => {
      const picked = await pickLocalBinaryFile('image/png,image/jpeg,image/webp,image/svg+xml');
      if (!picked) return;
      const mediaType = picked.ext === 'jpg' || picked.ext === 'jpeg' ? 'JPEG' : 'PNG';
      const res = await this.callEngine('insertCroppedImage', {
        mediaType,
        bytes: picked.bytes,
        xIn: 5.5,
        yIn: 4.25,
        cropIn: 0.08,
        brightness: 1.04,
        contrast: 1.06,
      });
      this.snapshot = res.snapshot;
      this.setSingleSelection(res.shapeId);
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.getElementById('btn-ins-ctrl-handle')?.addEventListener('click', async () => {
      if (!this.primarySelectedId) return;
      const res = await this.callEngine('ensureControlHandle', { shapeId: this.primarySelectedId });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.getElementById('btn-create-subprocess')?.addEventListener('click', async () => {
      if (!this.primarySelectedId) return;
      const res = await this.callEngine('createSubprocess', {
        shapeId: this.primarySelectedId,
        subName: '',
        navigate: true,
      });
      this.snapshot = res.snapshot;
      this.selectedShapeIds = [];
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    const jumpSubprocess = async (subPage?: string) => {
      if (!subPage) return;
      const res = await this.callEngine('selectPage', { pageName: subPage });
      this.snapshot = res.snapshot;
      this.selectedShapeIds = [];
      this.render();
    };
    document.getElementById('btn-jump-subprocess')?.addEventListener('click', (e) => {
      jumpSubprocess((e.currentTarget as HTMLElement).dataset.subPage);
    });
    document.querySelectorAll('[data-sub-pill]').forEach((btn) => {
      btn.addEventListener('pointerdown', (e) => e.stopPropagation());
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        jumpSubprocess((e.currentTarget as HTMLElement).dataset.subPill);
      });
    });

    document.getElementById('btn-ins-container')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertContainer', {
        title: 'Container',
        memberIds: this.selectedShapeIds,
      });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      if (res.containerId > 0) {
        this.setSingleSelection(res.containerId);
      }
      this.render();
    });

    document.getElementById('btn-ins-callout')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertCallout', {
        targetShapeId: this.primarySelectedId ?? 0,
        text: 'Callout Note',
      });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      if (res.calloutId > 0) {
        this.setSingleSelection(res.calloutId);
      }
      this.render();
    });

    document.getElementById('btn-ins-swimlanes')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertSwimlanes', {
        lanes: '',
      });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      this.render();
    });

    document.getElementById('btn-ins-room')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertRoom', { roomName: 'Main Data Hall' });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      this.setSingleSelection(res.roomId);
      this.render();
    });

    document.getElementById('btn-ins-rack')?.addEventListener('click', async () => {
      const res = await this.callEngine('insertRack');
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      this.setSingleSelection(res.rackId);
      this.render();
    });

    document.querySelectorAll('[data-layout]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const algorithm = (btn as HTMLElement).dataset.layout ?? 'tb';
        const res = await this.callEngine('autoLayout', { algorithm });
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects(res.wal);
        this.render();
      });
    });

    document.querySelectorAll('[data-pagesize]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const [w, h, j] = ((btn as HTMLElement).dataset.pagesize ?? '11,8.5,0').split(',').map(Number);
        const res = await this.callEngine('setPageDimensions', { widthIn: w, heightIn: h, jumpStyle: j });
        this.snapshot = res.snapshot;
        if (this.wgpuAttached) {
          const dpi = window.devicePixelRatio || 1;
          await this.callEngine('resizeOffscreenCanvas', {
            width: Math.round(w * 96 * dpi),
            height: Math.round(h * 96 * dpi),
            dpi,
          });
        }
        this.persistMutationSideEffects(res.wal);
        this.render();
      });
    });

    document.querySelectorAll('[data-validate]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const ruleset = (btn as HTMLElement).dataset.validate ?? 'bpmn';
        this.snapshot = (await this.callEngine('validate', { ruleset })).snapshot;
        this.activeInspectorTab = 'validate';
        this.render();
      });
    });

    document.getElementById('btn-gen-pert')?.addEventListener('click', async () => {
      await this.callEngine('addPage', { name: 'PERT-CriticalPath' });
      const res = await this.callEngine('generatePert');
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.getElementById('btn-gen-gantt')?.addEventListener('click', async () => {
      await this.callEngine('addPage', { name: 'Gantt-Schedule' });
      const res = await this.callEngine('generateGanttDemo');
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.getElementById('btn-gen-timeline')?.addEventListener('click', async () => {
      await this.callEngine('addPage', { name: 'Project-Timeline' });
      const res = await this.callEngine('generateTimelineDemo');
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.getElementById('btn-export-bom')?.addEventListener('click', async () => {
      const { bytes } = await this.callEngine('exportBomXlsx');
      await saveLocalDiagramInPlace(null, 'Vexio-BOM-Inventory-Report.xlsx', bytes);
    });

    document.getElementById('btn-add-quick-comment')?.addEventListener('click', () => {
      this.activeInspectorTab = 'comments';
      this.render();
    });

    document.getElementById('btn-open-layers')?.addEventListener('click', () => {
      this.activeInspectorTab = 'layers';
      this.render();
    });

    document.getElementById('btn-theme-toggle')?.addEventListener('click', () => {
      this.uiTheme = this.uiTheme === 'light' ? 'dark' : 'light';
      document.documentElement.dataset.theme = this.uiTheme;
      this.render();
    });

    const toggleLeftStencils = () => {
      this.showLeftStencils = !this.showLeftStencils;
      this.render();
    };
    const toggleRightInspector = () => {
      this.showRightInspector = !this.showRightInspector;
      this.render();
    };
    const toggleShapeSheet = () => {
      this.showShapeSheet = !this.showShapeSheet;
      this.render();
    };
    const toggleFormulaBar = () => {
      this.showFormulaBar = !this.showFormulaBar;
      this.render();
    };

    document.getElementById('btn-strip-stencils')?.addEventListener('click', toggleLeftStencils);
    document.getElementById('btn-view-stencils')?.addEventListener('click', toggleLeftStencils);
    document.getElementById('btn-strip-inspector')?.addEventListener('click', toggleRightInspector);
    document.getElementById('btn-view-inspector')?.addEventListener('click', toggleRightInspector);
    document.getElementById('btn-toggle-ss')?.addEventListener('click', toggleShapeSheet);
    document.getElementById('btn-dev-ss')?.addEventListener('click', toggleShapeSheet);
    document.getElementById('btn-strip-ss')?.addEventListener('click', toggleShapeSheet);
    document.getElementById('btn-toggle-fx-bar')?.addEventListener('click', toggleFormulaBar);
    document.getElementById('btn-dev-fx-bar')?.addEventListener('click', toggleFormulaBar);
    document.getElementById('btn-close-ss')?.addEventListener('click', () => {
      this.showShapeSheet = false;
      this.render();
    });

    const toggleMinimap = () => {
      this.showMinimap = !this.showMinimap;
      this.render();
    };
    document.getElementById('btn-toggle-minimap')?.addEventListener('click', toggleMinimap);
    document.getElementById('btn-zoom-map-pill')?.addEventListener('click', toggleMinimap);

    document.getElementById('btn-toggle-rulers')?.addEventListener('click', () => {
      this.showRulers = !this.showRulers;
      this.render();
    });

    document.getElementById('btn-toggle-grid')?.addEventListener('click', () => {
      this.showGrid = !this.showGrid;
      this.render();
    });

    document.getElementById('btn-toggle-smart-guides')?.addEventListener('click', () => {
      this.showSmartGuides = !this.showSmartGuides;
      this.latestSmartGuides = [];
      this.latestSnappedPin = null;
      this.render();
    });

    document.getElementById('chk-insp-grid')?.addEventListener('change', (e) => {
      this.showGrid = (e.target as HTMLInputElement).checked;
      this.render();
    });
    document.getElementById('chk-insp-rulers')?.addEventListener('change', (e) => {
      this.showRulers = (e.target as HTMLInputElement).checked;
      this.render();
    });
    document.getElementById('chk-insp-smart-guides')?.addEventListener('change', (e) => {
      this.showSmartGuides = (e.target as HTMLInputElement).checked;
      this.latestSmartGuides = [];
      this.latestSnappedPin = null;
      this.render();
    });
    document.getElementById('chk-insp-minimap')?.addEventListener('change', (e) => {
      this.showMinimap = (e.target as HTMLInputElement).checked;
      this.render();
    });
    document.getElementById('chk-insp-fx')?.addEventListener('change', (e) => {
      this.showFormulaBar = (e.target as HTMLInputElement).checked;
      this.render();
    });

    document.getElementById('btn-fit-window')?.addEventListener('click', () => {
      this.zoomPct = this.computeFitZoomPct();
      this.render();
    });
    document.getElementById('btn-zoom-reset')?.addEventListener('click', () => {
      this.zoomPct = this.computeFitZoomPct();
      this.render();
    });
    document.getElementById('btn-zoom-100')?.addEventListener('click', () => {
      this.zoomPct = 100;
      this.render();
    });
    document.getElementById('btn-zoom-100-pill')?.addEventListener('click', () => {
      this.zoomPct = 100;
      this.render();
    });
    document.getElementById('btn-zoom-in')?.addEventListener('click', () => {
      this.zoomPct = Math.min(400, this.zoomPct + 15);
      this.render();
    });
    document.getElementById('btn-zoom-out')?.addEventListener('click', () => {
      this.zoomPct = Math.max(25, this.zoomPct - 15);
      this.render();
    });

    const vpEl = document.getElementById('canvas-viewport');
    if (vpEl) {
      vpEl.addEventListener('scroll', () => this.syncViewportRulersAndMinimap(), { passive: true });
      vpEl.addEventListener(
        'wheel',
        (ev) => {
          if (ev.ctrlKey || ev.metaKey) {
            ev.preventDefault();
            const delta = ev.deltaY < 0 ? 10 : -10;
            this.zoomPct = Math.max(25, Math.min(400, this.zoomPct + delta));
            this.render();
          }
        },
        { passive: false }
      );
    }

    const mmStage = document.getElementById('minimap-stage');
    if (mmStage && vpEl) {
      const panFromMinimap = (ev: PointerEvent | MouseEvent) => {
        const r = mmStage.getBoundingClientRect();
        const u = Math.max(0, Math.min(1, (ev.clientX - r.left) / Math.max(1, r.width)));
        const v = Math.max(0, Math.min(1, (ev.clientY - r.top) / Math.max(1, r.height)));
        vpEl.scrollLeft = Math.max(0, u * vpEl.scrollWidth - vpEl.clientWidth * 0.5);
        vpEl.scrollTop = Math.max(0, v * vpEl.scrollHeight - vpEl.clientHeight * 0.5);
        this.syncViewportRulersAndMinimap();
      };
      mmStage.addEventListener('pointerdown', (ev) => {
        ev.stopPropagation();
        mmStage.setPointerCapture(ev.pointerId);
        panFromMinimap(ev);
      });
      mmStage.addEventListener('pointermove', (ev) => {
        if (ev.buttons === 1) panFromMinimap(ev);
      });
    }

    document.getElementById('new-cell-name')?.addEventListener('change', (e) => {
      this.activeFormulaCell = (e.target as HTMLInputElement).value.trim() || 'Width';
      this.render();
    });
    document.getElementById('new-cell-formula')?.addEventListener('input', (e) => {
      this.activeFormulaDraft = (e.target as HTMLInputElement).value;
    });
    document.getElementById('new-cell-formula')?.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        this.applyFormulaFromStrip();
      }
    });
    document.querySelectorAll('.ss-table tbody tr[data-ss-cell]').forEach((row) => {
      row.addEventListener('click', () => {
        const el = row as HTMLElement;
        this.activeFormulaCell = el.dataset.ssCell ?? 'Width';
        this.activeFormulaDraft = el.dataset.ssFormula ?? '';
        const nameInp = document.getElementById('new-cell-name') as HTMLInputElement | null;
        const fxInp = document.getElementById('new-cell-formula') as HTMLInputElement | null;
        if (nameInp) nameInp.value = this.activeFormulaCell;
        if (fxInp) {
          fxInp.value = this.activeFormulaDraft;
          fxInp.focus();
        }
      });
    });

    document.getElementById('btn-recover-opfs')?.addEventListener('click', async () => {
      const res = await this.callOpfs('readWal', { docId: this.docId });
      if (res.bytes && res.bytes.byteLength > 12) {
        this.snapshot = (await this.callEngine('replayWal', { bytes: res.bytes })).snapshot;
        this.showToast('Recovered OPFS Write-Ahead Log', 'info');
        this.render();
      } else {
        this.showToast('No pending OPFS WAL records to replay.', 'info');
      }
    });

    // Preserve Modal Drafts on every keystroke so background re-renders never wipe user edits
    document.getElementById('vba-editor')?.addEventListener('input', (e) => {
      this.vbaDraft = (e.target as HTMLTextAreaElement).value;
    });
    document.getElementById('sql-editor')?.addEventListener('input', (e) => {
      this.sqlDraft = (e.target as HTMLTextAreaElement).value;
    });
    document.getElementById('csv-editor')?.addEventListener('input', (e) => {
      this.csvDraft = (e.target as HTMLTextAreaElement).value;
    });
    document.getElementById('org-csv-editor')?.addEventListener('input', (e) => {
      this.orgCsvDraft = (e.target as HTMLTextAreaElement).value;
    });
    document.getElementById('dataviz-editor')?.addEventListener('input', (e) => {
      this.dataVizDraft = (e.target as HTMLTextAreaElement).value;
    });
    document.getElementById('cloud-json-editor')?.addEventListener('input', (e) => {
      this.cloudJsonDraft = (e.target as HTMLTextAreaElement).value;
    });
    document.getElementById('version-label-input')?.addEventListener('input', (e) => {
      this.versionLabelDraft = (e.target as HTMLInputElement).value;
    });
    document.getElementById('link-addr-input')?.addEventListener('input', (e) => {
      this.linkAddressDraft = (e.target as HTMLInputElement).value;
    });
    document.getElementById('link-sub-input')?.addEventListener('input', (e) => {
      this.linkSubAddressDraft = (e.target as HTMLInputElement).value;
    });
    document.getElementById('link-tip-input')?.addEventListener('input', (e) => {
      this.linkScreentipDraft = (e.target as HTMLInputElement).value;
    });

    // Studio & Backstage Modals
    document.getElementById('btn-open-vba-modal')?.addEventListener('click', () => {
      this.activeModal = 'vba';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-sql-modal')?.addEventListener('click', () => {
      this.activeModal = 'sql';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-data-modal')?.addEventListener('click', () => {
      this.activeModal = 'data';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-dataviz-modal')?.addEventListener('click', () => {
      this.activeModal = 'dataviz';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-cloud-modal')?.addEventListener('click', () => {
      this.activeModal = 'cloud';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-compare-modal')?.addEventListener('click', async () => {
      this.activeModal = 'compare';
      this.modalOutputMessage = '';
      try {
        const vRes = await this.callOpfs('listVersions', { docId: this.docId });
        this.opfsVersions = vRes.versions ?? [];
      } catch {
        // Offline ignore
      }
      this.render();
    });
    document.getElementById('btn-open-link-modal')?.addEventListener('click', () => {
      this.activeModal = 'link';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-open-org-modal')?.addEventListener('click', () => {
      this.activeModal = 'org';
      this.modalOutputMessage = '';
      this.render();
    });
    document.getElementById('btn-close-modal')?.addEventListener('click', () => {
      this.activeModal = null;
      this.render();
    });

    // Backstage Template Cards, Version Snapshots & Workspaces
    document.querySelectorAll('[data-template-id]').forEach((card) => {
      card.addEventListener('click', async () => {
        const tid = (card as HTMLElement).dataset.templateId ?? 'blank';
        const tpl = STARTER_TEMPLATES.find((t) => t.id === tid);
        const res = await this.callEngine('loadStarterTemplate', { templateId: tid });
        this.currentFileName = `${(tpl?.title ?? 'Diagram').replace(/[^a-zA-Z0-9-]+/g, '-')}.vsdx`;
        this.snapshot = res.snapshot;
        this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
        this.activeModal = null;
        this.persistMutationSideEffects(res.wal);
        this.render();
      });
    });

    document.getElementById('btn-create-version')?.addEventListener('click', async () => {
      const inp = document.getElementById('version-label-input') as HTMLInputElement | null;
      this.versionLabelDraft = inp?.value.trim() || 'Milestone Checkpoint';
      await this.saveNewVersionSnapshot(this.versionLabelDraft);
      this.modalOutputMessage = `Saved immutable snapshot "${this.versionLabelDraft}" to OPFS${this.cloudState.user ? ' + Firebase Cloud' : ''}.`;
      this.render();
    });

    document.querySelectorAll('[data-restore-version]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const versionId = (btn as HTMLElement).dataset.restoreVersion!;
        const loaded = await this.callOpfs('loadVersion', { docId: this.docId, versionId });
        if (loaded.bytes && loaded.bytes.byteLength > 0) {
          const res = await this.callEngine('openBytes', { bytes: loaded.bytes, ext: 'vsdx' });
          this.snapshot = res.snapshot;
          this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
          this.activeModal = null;
          this.persistMutationSideEffects();
          this.render();
        }
      });
    });

    document.getElementById('btn-new-workspace')?.addEventListener('click', async () => {
      const slug = `workspace-${Date.now().toString(36).slice(-5)}`;
      this.docId = slug;
      this.currentFileName = `${slug}.vsdx`;
      this.collabSession.setRoom(this.docId);
      const res = await this.callEngine('loadStarterTemplate', { templateId: 'blank' });
      this.snapshot = res.snapshot;
      this.setSingleSelection(null);
      this.activeModal = null;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.querySelectorAll('[data-open-opfs-ws]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const el = btn as HTMLElement;
        const targetId = el.dataset.openOpfsWs!;
        const loaded = await this.callOpfs('loadWorkspace', { docId: targetId });
        if (loaded.bytes && loaded.bytes.byteLength > 0) {
          this.docId = targetId;
          this.currentFileName = el.dataset.wsTitle ?? `${targetId}.vsdx`;
          this.collabSession.setRoom(this.docId);
          const res = await this.callEngine('openBytes', { bytes: loaded.bytes, ext: 'vsdx' });
          this.snapshot = res.snapshot;
          this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
          this.activeModal = null;
          this.render();
        }
      });
    });

    document.querySelectorAll('[data-open-cloud-ws]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        const el = btn as HTMLElement;
        const targetId = el.dataset.openCloudWs!;
        const bytes = await this.firebaseSync.restoreWorkspaceFromCloud(targetId, 'vsdx');
        if (bytes && bytes.byteLength > 0) {
          this.docId = targetId;
          this.currentFileName = el.dataset.wsTitle ?? `${targetId}.vsdx`;
          this.collabSession.setRoom(this.docId);
          const res = await this.callEngine('openBytes', { bytes, ext: 'vsdx' });
          this.snapshot = res.snapshot;
          this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
          this.activeModal = null;
          this.render();
        }
      });
    });

    document.getElementById('btn-import-dataviz-csv')?.addEventListener('click', async () => {
      this.dataVizDraft = (document.getElementById('dataviz-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'DataVisualizer-Flow' });
      const bytes = new TextEncoder().encode(this.dataVizDraft);
      const res = await this.callEngine('importDataVisualizer', { bytes });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Generated ${res.count} swimlanes, process shapes & routed connectors from Data Visualizer table.`;
      this.render();
    });

    document.getElementById('btn-pick-dataviz-xlsx')?.addEventListener('click', async () => {
      const picked = await pickLocalBinaryFile('.xlsx,.csv');
      if (!picked) return;
      await this.callEngine('addPage', { name: 'DataVisualizer-Workbook' });
      const res = await this.callEngine('importDataVisualizer', { bytes: picked.bytes });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Imported ${picked.name} (${res.count} shapes & swimlanes).`;
      this.render();
    });

    document.getElementById('btn-export-dataviz-xlsx')?.addEventListener('click', async () => {
      const { bytes } = await this.callEngine('exportDataVisualizer');
      await saveLocalDiagramInPlace(null, 'Vexio-DataVisualizer-ProcessMap.xlsx', bytes);
      this.modalOutputMessage = 'Exported bi-directional Data Visualizer .xlsx workbook.';
      this.render();
    });

    document.getElementById('btn-build-cloud-topology')?.addEventListener('click', async () => {
      this.cloudJsonDraft = (document.getElementById('cloud-json-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'Cloud-Topology' });
      const res = await this.callEngine('importCloudDiscovery', { json: this.cloudJsonDraft });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Generated ${res.count} VPC containers, cloud resources & TLS connectors.`;
      this.render();
    });

    document.getElementById('btn-compare-latest-opfs')?.addEventListener('click', async () => {
      const latest = this.opfsVersions[0];
      if (!latest) {
        this.modalOutputMessage = 'No OPFS snapshot saved yet—click "+ Snapshot" in the titlebar first, then edit shapes.';
        this.render();
        return;
      }
      const loaded = await this.callOpfs('loadVersion', { docId: this.docId, versionId: latest.versionId });
      if (loaded.bytes && loaded.bytes.byteLength > 0) {
        const res = await this.callEngine('compareWithVsdx', { bytes: loaded.bytes });
        this.latestDiffReport = res.report;
        this.modalOutputMessage = `Compared current workspace against snapshot "${latest.label}".`;
        this.render();
      }
    });

    document.getElementById('btn-compare-disk-vsdx')?.addEventListener('click', async () => {
      const picked = await pickLocalBinaryFile('.vsdx');
      if (!picked) return;
      const res = await this.callEngine('compareWithVsdx', { bytes: picked.bytes });
      this.latestDiffReport = res.report;
      this.modalOutputMessage = `Compared current workspace against local file "${picked.name}".`;
      this.render();
    });

    document.getElementById('btn-save-link-tip')?.addEventListener('click', async () => {
      if (!this.primarySelectedId) return;
      this.linkAddressDraft = (document.getElementById('link-addr-input') as HTMLInputElement).value;
      this.linkSubAddressDraft = (document.getElementById('link-sub-input') as HTMLInputElement).value;
      this.linkScreentipDraft = (document.getElementById('link-tip-input') as HTMLInputElement).value;
      const res = await this.callEngine('setHyperlinkAndScreentip', {
        shapeId: this.primarySelectedId,
        description: 'Architecture Link',
        address: this.linkAddressDraft,
        subAddress: this.linkSubAddressDraft,
        screentip: this.linkScreentipDraft,
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.activeModal = null;
      this.render();
    });

    document.getElementById('btn-build-org-chart')?.addEventListener('click', async () => {
      this.orgCsvDraft = (document.getElementById('org-csv-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'Org-Chart' });
      const res = await this.callEngine('importOrgChartCsv', { csv: this.orgCsvDraft });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Generated ${res.count} executive/team Org Chart cards with reporting hierarchy.`;
      this.render();
    });

    document.getElementById('btn-build-org-entra-ldif')?.addEventListener('click', async () => {
      this.orgCsvDraft = (document.getElementById('org-csv-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'Directory-Org' });
      const res = await this.callEngine('importEntraOrLdif', { text: this.orgCsvDraft });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Imported ${res.count} directory entries from Entra Graph JSON / LDIF.`;
      this.render();
    });

    document.getElementById('btn-exec-vba')?.addEventListener('click', async () => {
      this.vbaDraft = (document.getElementById('vba-editor') as HTMLTextAreaElement).value;
      const res = await this.callEngine('runMacro', {
        script: this.vbaDraft,
        selectedIds: this.selectedShapeIds,
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Executed ${res.macroReport.instructions_executed} sandboxed instructions (${res.macroReport.shapes_created} shapes created).`;
      this.render();
    });

    document.getElementById('btn-reverse-sql')?.addEventListener('click', async () => {
      this.sqlDraft = (document.getElementById('sql-editor') as HTMLTextAreaElement).value;
      await this.callEngine('addPage', { name: 'ERD-Schema' });
      const res = await this.callEngine('reverseSql', { sql: this.sqlDraft });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Reverse-engineered ${res.count} SQL tables into Crow's Foot ERD shapes & FK connectors.`;
      this.render();
    });

    document.getElementById('btn-forward-sql')?.addEventListener('click', async () => {
      const res = await this.callEngine('forwardSql');
      if (res.sql && res.sql.trim().length > 0) {
        this.sqlDraft = res.sql;
        this.modalOutputMessage = 'Generated SQL CREATE TABLE DDL from active ERD page.';
      } else {
        this.modalOutputMessage = 'No ERD entity tables on current page yet—click Reverse-Engineer first.';
      }
      this.render();
    });

    document.getElementById('btn-apply-csv-dg')?.addEventListener('click', async () => {
      this.csvDraft = (document.getElementById('csv-editor') as HTMLTextAreaElement).value;
      const res = await this.callEngine('linkCsvDataGraphics', {
        csv: this.csvDraft,
        keyCol: 'Label',
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Linked ${res.linked} shapes to External Data & applied Data Graphics + Page Legend!`;
      this.render();
    });

    document.getElementById('btn-bind-sqlite-file')?.addEventListener('click', async () => {
      const picked = await pickLocalBinaryFile('.sqlite,.db,.sqlite3');
      if (!picked) return;
      const res = await this.callEngine('importSqliteRecordset', {
        bytes: picked.bytes,
        tableName: '',
        keyCol: 'Label',
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.modalOutputMessage = `Bound ${res.count} shapes directly from offline SQLite B-tree database "${picked.name}".`;
      this.render();
    });

    // Stencils: 22-Category Accordions, Quick Domain Filter Pills, Cross-Pack Search, Click, Right-Click & Drag-and-Drop!
    document.querySelectorAll('[data-stencil-group]').forEach((pill) => {
      pill.addEventListener('click', () => {
        const gid = (pill as HTMLElement).dataset.stencilGroup as
          | 'all'
          | 'microsoft'
          | 'network'
          | 'basic'
          | 'cloud'
          | 'eng'
          | undefined;
        if (!gid) return;
        this.stencilGroupFilter = gid;
        this.render();
      });
    });

    document.querySelectorAll('[data-acc-pack]').forEach((hdr) => {
      hdr.addEventListener('click', () => {
        const pack = (hdr as HTMLElement).dataset.accPack;
        if (!pack) return;
        if (this.stencilGroupFilter !== 'all') {
          this.stencilGroupFilter = 'all';
          this.expandedStencilPacks.add(pack);
        } else if (this.expandedStencilPacks.has(pack)) {
          this.expandedStencilPacks.delete(pack);
        } else {
          this.expandedStencilPacks.add(pack);
        }
        this.activeStencilPack = pack;
        this.render();
      });
    });

    const importVssxFile = async (file: File) => {
      try {
        const bytes = new Uint8Array(await file.arrayBuffer());
        const opfsCopy = new Uint8Array(bytes);
        this.callOpfs('cacheStencil', { name: file.name, vssxBytes: opfsCopy }, [opfsCopy.buffer]).catch(() => {});
        const res = await this.callEngine(
          'importStencil',
          { fileName: file.name, bytes },
          [bytes.buffer]
        );
        if (res.packTitle && Array.isArray(res.masterNames) && res.masterNames.length > 0) {
          this.stencilCatalog = {
            [res.packTitle]: res.masterNames,
            ...this.stencilCatalog,
          };
          this.stencilPreviews = {
            [res.packTitle]: res.previews ?? [],
            ...this.stencilPreviews,
          };
          this.packOrder = [res.packTitle, ...this.packOrder.filter((p) => p !== res.packTitle)];
          this.expandedStencilPacks.add(res.packTitle);
          this.stencilGroupFilter = 'all';
          this.showToast(
            `Imported Visio stencil "${res.packTitle}" (${res.masterNames.length} masters)`,
            'info'
          );
          this.render();
        } else {
          this.showToast(`No stencil masters found in "${file.name}".`, 'warn');
        }
      } catch (err) {
        this.showToast(
          `Stencil import error: ${err instanceof Error ? err.message : 'invalid .vssx/.vss file'}`,
          'error'
        );
      }
    };

    document.getElementById('btn-import-vssx')?.addEventListener('click', () => {
      (document.getElementById('vssx-file-input') as HTMLInputElement | null)?.click();
    });
    document.getElementById('vssx-file-input')?.addEventListener('change', (ev) => {
      const f = (ev.target as HTMLInputElement).files?.[0];
      if (f) importVssxFile(f);
    });
    const stencilAside = document.querySelector('.stencil-pane') as HTMLElement | null;
    if (stencilAside) {
      stencilAside.addEventListener('dragover', (ev) => {
        if (ev.dataTransfer?.types.includes('Files')) ev.preventDefault();
      });
      stencilAside.addEventListener('drop', (ev) => {
        const f = ev.dataTransfer?.files?.[0];
        if (f && /\.(vssx|vssm|vss|vsdx|vstx|vstm|vsd|vst|vdx)$/i.test(f.name)) {
          ev.preventDefault();
          ev.stopPropagation();
          importVssxFile(f);
        }
      });
    }

    document.getElementById('btn-expand-collapse-packs')?.addEventListener('click', () => {
      const allPacks = Object.keys(this.stencilCatalog);
      if (this.expandedStencilPacks.size >= allPacks.length) {
        this.expandedStencilPacks = new Set([
          'Microsoft 365 & Office',
          'Servers (Standard Roles)',
          'Network & Rack Equipment',
        ]);
      } else {
        this.expandedStencilPacks = new Set(allPacks);
      }
      this.render();
    });

    document.getElementById('btn-clear-stencil-search')?.addEventListener('click', () => {
      this.stencilFilter = '';
      this.render();
    });
    document.getElementById('stencil-search')?.addEventListener('input', (e) => {
      const inp = e.target as HTMLInputElement;
      const caret = inp.selectionStart ?? inp.value.length;
      this.stencilFilter = inp.value;
      this.render();
      const nextInp = document.getElementById('stencil-search') as HTMLInputElement | null;
      if (nextInp) {
        nextInp.focus();
        nextInp.setSelectionRange(caret, caret);
      }
    });
    document.querySelectorAll('.stencil-item').forEach((item) => {
      const el = item as HTMLElement;
      const packCategory = el.dataset.pack ?? this.activeStencilPack;
      const masterName = el.dataset.master ?? 'Process';
      el.addEventListener('dragstart', (ev) => {
        ev.dataTransfer?.setData(
          'application/x-vexio-stencil',
          JSON.stringify({ category: packCategory, masterName })
        );
      });
      el.addEventListener('contextmenu', (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        this.contextMenu = {
          kind: 'stencil',
          x: ev.clientX,
          y: ev.clientY,
          stencilPack: packCategory,
          stencilMaster: masterName,
        };
        this.render();
      });
      el.addEventListener('click', async () => {
        const count = this.snapshot?.shapes.length ?? 0;
        const res = await this.callEngine('dropShape', {
          category: packCategory,
          masterName,
          xIn: 2.2 + (count % 4) * 2.1,
          yIn: 6.5 - Math.floor(count / 4) * 1.6,
        });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.activeTool = 'pointer';
        this.setSingleSelection(res.shapeId);
        this.render();
      });
    });

    // Inline Double-Click / F2 Text Editor Listeners
    const inlineEditor = document.getElementById('inline-shape-editor') as HTMLTextAreaElement | null;
    if (inlineEditor) {
      let committed = false;
      const commitInlineText = async () => {
        if (committed || this.editingShapeId === null) return;
        committed = true;
        const targetId = this.editingShapeId;
        const textVal = inlineEditor.value;
        this.editingShapeId = null;
        const res = await this.callEngine('setShapeText', { shapeId: targetId, text: textVal });
        this.persistMutationSideEffects(res.wal);
        this.snapshot = res.snapshot;
        this.render();
      };
      inlineEditor.addEventListener('pointerdown', (e) => {
        e.stopPropagation();
        this.inlineSelectAllGuardUntilMs = 0;
      });
      inlineEditor.addEventListener('pointerup', (e) => e.stopPropagation());
      inlineEditor.addEventListener('mouseup', (e) => {
        e.stopPropagation();
        if (performance.now() < this.inlineSelectAllGuardUntilMs) {
          e.preventDefault();
          inlineEditor.setSelectionRange(0, inlineEditor.value.length);
        }
      });
      inlineEditor.addEventListener('click', (e) => {
        e.stopPropagation();
        if (performance.now() < this.inlineSelectAllGuardUntilMs) {
          e.preventDefault();
          inlineEditor.setSelectionRange(0, inlineEditor.value.length);
        }
      });
      inlineEditor.addEventListener('dblclick', (e) => {
        e.stopPropagation();
        if (performance.now() < this.inlineSelectAllGuardUntilMs) {
          e.preventDefault();
          inlineEditor.setSelectionRange(0, inlineEditor.value.length);
          this.inlineSelectAllGuardUntilMs = 0;
        }
      });
      inlineEditor.addEventListener('input', () => {
        this.inlineSelectAllGuardUntilMs = 0;
        this.inlineTextDraft = inlineEditor.value;
      });
      inlineEditor.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault();
          e.stopPropagation();
          commitInlineText();
        } else if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          committed = true;
          this.editingShapeId = null;
          this.render();
        }
      });
      inlineEditor.addEventListener('blur', () => {
        if (this.isRebuildingDom || !inlineEditor.isConnected) return;
        commitInlineText();
      });
    }

    // Direct-Manipulation Canvas Pointer (100% Synchronous pointerdown, Right-Click Context Menus, All Basic Tools)
    const host = document.getElementById('diagram-host');
    if (host) {
      host.addEventListener('contextmenu', (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);
        const hitId = this.hitTestSnapshotSync(ptIn[0], ptIn[1], false, true);
        if (hitId > 0) {
          if (!this.selectedShapeIds.includes(hitId)) {
            this.setSingleSelection(hitId);
          }
          this.contextMenu = {
            kind: 'shape',
            x: ev.clientX,
            y: ev.clientY,
            pageIn: ptIn,
          };
        } else {
          this.contextMenu = {
            kind: 'canvas',
            x: ev.clientX,
            y: ev.clientY,
            pageIn: ptIn,
          };
        }
        this.render();
      });

      host.addEventListener('dragover', (ev) => ev.preventDefault());
      host.addEventListener('drop', async (ev) => {
        ev.preventDefault();
        const raw = ev.dataTransfer?.getData('application/x-vexio-stencil');
        if (raw) {
          const { category, masterName } = JSON.parse(raw);
          const [xIn, yIn] = this.clientToPageInches(host, ev.clientX, ev.clientY);
          const res = await this.callEngine('dropShape', { category, masterName, xIn, yIn });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.activeTool = 'pointer';
          this.setSingleSelection(res.shapeId);
          this.render();
          return;
        }
        const droppedFile = ev.dataTransfer?.files?.[0];
        if (droppedFile) {
          const ext = droppedFile.name.split('.').pop()?.toLowerCase() ?? 'vsdx';
          if (['vssx', 'vssm', 'vss', 'vst'].includes(ext)) {
            await importVssxFile(droppedFile);
            return;
          }
          if (['vsdx', 'vstx', 'vstm', 'vsdm', 'vsd', 'vdx', 'vdw', 'dwg', 'dxf'].includes(ext)) {
            const bytes = new Uint8Array(await droppedFile.arrayBuffer());
            this.activeFileHandle = null;
            this.currentFileName = droppedFile.name;
            const res = await this.callEngine('openBytes', { bytes, ext });
            this.snapshot = res.snapshot;
            this.activeModal = null;
            await this.syncCanvasAfterDocumentOpen();
            this.setSingleSelection(this.snapshot?.shapes[0]?.id ?? null);
            this.persistMutationSideEffects();
            this.showToast(`Opened dropped file "${droppedFile.name}"`, 'info');
            this.render();
          }
        }
      });

      host.addEventListener('dblclick', (ev) => {
        const targetEl = ev.target as HTMLElement | null;
        if (targetEl?.id === 'inline-shape-editor') return;
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);
        const hitId = this.hitTestSnapshotSync(ptIn[0], ptIn[1], false, true);
        if (hitId > 0 && this.editingShapeId !== hitId) {
          ev.preventDefault();
          ev.stopPropagation();
          this.lastCanvasClick = null;
          this.openInlineShapeTextEditor(hitId);
        }
      });

      const cancelActiveGesture = () => {
        if (this.dragState) {
          this.dragState = null;
          host.classList.remove('panning');
          const ov = document.getElementById('gesture-overlay-svg');
          if (ov) ov.innerHTML = '';
        }
      };
      host.addEventListener('pointercancel', cancelActiveGesture);
      host.addEventListener('lostpointercapture', cancelActiveGesture);

      host.addEventListener('pointerdown', (ev) => {
        if (this.contextMenu) {
          this.contextMenu = null;
          document.getElementById('vexio-context-menu')?.remove();
        }

        const targetEl = ev.target as HTMLElement | null;
        if (targetEl?.id === 'inline-shape-editor' || targetEl?.dataset.subPill) {
          return;
        }

        // 0. Deterministic Double-Click / Double-Tap Stencil Text Edit Detection
        // Survives innerHTML DOM rebuilds after Click #1 and takes priority over nearby grips/ports on compact stencils!
        if (ev.button === 0 && this.activeTool === 'pointer' && !this.spaceKeyDown && !ev.shiftKey) {
          const ptCheck = this.clientToPageInches(host, ev.clientX, ev.clientY);
          const hitCheck = this.hitTestSnapshotSync(ptCheck[0], ptCheck[1], false, true);
          const nowMs = performance.now();
          const isDoubleActivate =
            hitCheck > 0 &&
            (ev.detail >= 2 ||
              (this.lastCanvasClick !== null &&
                this.lastCanvasClick.shapeId === hitCheck &&
                nowMs - this.lastCanvasClick.timeMs <= 500 &&
                Math.hypot(
                  ev.clientX - this.lastCanvasClick.clientX,
                  ev.clientY - this.lastCanvasClick.clientY
                ) <= 24));
          if (isDoubleActivate) {
            ev.preventDefault();
            ev.stopPropagation();
            this.lastCanvasClick = null;
            host.setPointerCapture(ev.pointerId);
            this.dragState = {
              mode: 'open_text_edit',
              shapeId: hitCheck,
              startPageIn: ptCheck,
              curPageIn: ptCheck,
            };
            return;
          }
          this.lastCanvasClick =
            hitCheck > 0
              ? { shapeId: hitCheck, timeMs: nowMs, clientX: ev.clientX, clientY: ev.clientY }
              : null;
        }

        // Pan via Middle-Mouse, Hand tool, or Spacebar+Drag
        if (ev.button === 1 || (ev.button === 0 && (this.activeTool === 'pan' || this.spaceKeyDown))) {
          ev.preventDefault();
          host.setPointerCapture(ev.pointerId);
          host.classList.add('panning');
          const vp = document.getElementById('canvas-viewport');
          const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);
          this.dragState = {
            mode: 'pan',
            startPageIn: ptIn,
            curPageIn: ptIn,
            panStartClient: [ev.clientX, ev.clientY],
            panStartScroll: [vp?.scrollLeft ?? 0, vp?.scrollTop ?? 0],
          };
          return;
        }

        if (ev.button !== 0) return;
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);

        // 0A. Check Draggable Yellow Control Handle Diamond
        const ctrlShapeAttr = targetEl?.dataset.ctrlShape;
        if (ctrlShapeAttr) {
          ev.stopPropagation();
          host.setPointerCapture(ev.pointerId);
          this.dragState = {
            mode: 'control',
            startPageIn: ptIn,
            curPageIn: ptIn,
            shapeId: Number(ctrlShapeAttr),
            controlRowName: targetEl?.dataset.ctrlRow ?? 'Row_1',
          };
          return;
        }

        // 0B. Check Vertex / NURBS Edit Grip
        const vtxShapeAttr = targetEl?.dataset.vtxShape;
        if (vtxShapeAttr) {
          ev.stopPropagation();
          host.setPointerCapture(ev.pointerId);
          this.dragState = {
            mode: 'vertex',
            startPageIn: ptIn,
            curPageIn: ptIn,
            shapeId: Number(vtxShapeAttr),
            geomIndex: Number(targetEl?.dataset.vtxGeom ?? 0),
            vertexRowIx: Number(targetEl?.dataset.vtxRow ?? 1),
          };
          return;
        }

        // 0C. Check 1D Connector Begin/End Endpoint Grip OR Interior Orthogonal Segment Diamond
        const segShapeAttr = targetEl?.dataset.segShape;
        if (segShapeAttr) {
          ev.stopPropagation();
          host.setPointerCapture(ev.pointerId);
          this.dragState = {
            mode: 'conn_segment',
            startPageIn: ptIn,
            curPageIn: ptIn,
            shapeId: Number(segShapeAttr),
            segIndex: Number(targetEl?.dataset.segIndex ?? 1),
            segAxis: (targetEl?.dataset.segAxis === 'y' ? 'y' : 'x') as 'x' | 'y',
          };
          return;
        }

        const connShapeAttr = targetEl?.dataset.connShape;
        if (connShapeAttr) {
          ev.stopPropagation();
          host.setPointerCapture(ev.pointerId);
          this.dragState = {
            mode: 'conn_endpoint',
            startPageIn: ptIn,
            curPageIn: ptIn,
            shapeId: Number(connShapeAttr),
            connIsBegin: targetEl?.dataset.connEnd === 'begin',
          };
          return;
        }

        // 0D. Check 2D Cardinal Connection Port Dot (Drag from any port to draw glued connector!)
        // 1. Check Top Rotation Knob FIRST
        const rotEl = targetEl?.closest('[data-rotate-knob]') as HTMLElement | null;
        const rotIdAttr = rotEl?.dataset.rotateKnob;
        if (rotIdAttr && this.snapshot) {
          const sid = Number(rotIdAttr);
          const sh = this.snapshot.shapes.find((s) => s.id === sid);
          if (sh) {
            ev.stopPropagation();
            host.setPointerCapture(ev.pointerId);
            this.dragState = {
              mode: 'rotate',
              startPageIn: ptIn,
              curPageIn: ptIn,
              shapeId: sid,
              origPin: [sh.pinX, sh.pinY],
              origSize: [sh.width, sh.height],
              origAngleDeg: sh.angleDeg,
            };
            return;
          }
        }

        // 2. Check 8-Handle Resize Grips (DOM closest + synchronous geometric proximity fallback!)
        const handleEl = targetEl?.closest('[data-handle]') as HTMLElement | null;
        let handleDir = handleEl?.dataset.handle as ResizeHandleDir | undefined;
        let handleShapeId = handleEl?.dataset.handleShape
          ? Number(handleEl.dataset.handleShape)
          : this.primarySelectedId;
        if (!handleDir && this.activeTool === 'pointer' && this.primarySelectedId && this.snapshot) {
          const geomHit = this.hitTestResizeGripSync(ptIn[0], ptIn[1]);
          if (geomHit) {
            handleDir = geomHit.dir;
            handleShapeId = geomHit.shapeId;
          }
        }
        if (handleDir && handleShapeId && this.snapshot) {
          const sh = this.snapshot.shapes.find((s) => s.id === handleShapeId);
          if (sh) {
            ev.stopPropagation();
            host.setPointerCapture(ev.pointerId);
            this.dragState = {
              mode: 'resize',
              startPageIn: ptIn,
              curPageIn: ptIn,
              shapeId: sh.id,
              origPin: [sh.pinX, sh.pinY],
              origSize: [sh.width, sh.height],
              origAngleDeg: sh.angleDeg,
              resizeHandle: handleDir,
            };
            return;
          }
        }

        // 3. Check 2D Connection Port Dot / AutoConnect Arrow
        const portEl = targetEl?.closest('[data-port-shape]') as HTMLElement | null;
        const portShapeAttr = portEl?.dataset.portShape;
        if (portShapeAttr) {
          ev.stopPropagation();
          host.setPointerCapture(ev.pointerId);
          const px = Number(portEl?.dataset.portX ?? ptIn[0]);
          const py = Number(portEl?.dataset.portY ?? ptIn[1]);
          const acDir = portEl?.dataset.autoconnectDir;
          this.dragState = {
            mode: 'connect',
            startPageIn: acDir ? ptIn : [px, py],
            curPageIn: ptIn,
            shapeId: Number(portShapeAttr),
            origPin: [px, py],
            autoConnectDir: acDir,
          };
          return;
        }

        host.setPointerCapture(ev.pointerId);

        if (this.activeTool === 'pen' || this.activeTool === 'highlighter') {
          this.dragState = {
            mode: 'ink',
            startPageIn: ptIn,
            curPageIn: ptIn,
            inkPoints: [
              {
                x: ptIn[0],
                y: ptIn[1],
                pressure: ev.pressure || 0.5,
                tilt_x: ev.tiltX || 0,
                tilt_y: ev.tiltY || 0,
              },
            ],
          };
          return;
        }

        if (this.activeTool === 'freeform') {
          this.dragState = {
            mode: 'freeform',
            startPageIn: ptIn,
            curPageIn: ptIn,
            freeformPoints: [ptIn],
          };
          return;
        }

        if (this.activeTool === 'line') {
          this.dragState = { mode: 'line', startPageIn: ptIn, curPageIn: ptIn };
          return;
        }

        if (this.activeTool === 'arc') {
          this.dragState = { mode: 'arc', startPageIn: ptIn, curPageIn: ptIn };
          return;
        }

        if (this.activeTool === 'connector') {
          const from2D = this.hitTestSnapshotSync(ptIn[0], ptIn[1], true);
          this.dragState = {
            mode: 'connect',
            startPageIn: ptIn,
            curPageIn: ptIn,
            shapeId: from2D,
          };
          return;
        }

        if (
          this.activeTool === 'rect' ||
          this.activeTool === 'round_rect' ||
          this.activeTool === 'ellipse' ||
          this.activeTool === 'diamond' ||
          this.activeTool === 'triangle' ||
          this.activeTool === 'hexagon' ||
          this.activeTool === 'star' ||
          this.activeTool === 'text'
        ) {
          if (this.activeTool === 'text') {
            const hitExisting = this.hitTestSnapshotSync(ptIn[0], ptIn[1], false, true);
            if (hitExisting > 0 && this.snapshot) {
              this.activeTool = 'pointer';
              this.openInlineShapeTextEditor(hitExisting);
              return;
            }
          }
          this.dragState = {
            mode: 'primitive',
            primitiveKind: this.activeTool,
            startPageIn: ptIn,
            curPageIn: ptIn,
          };
          return;
        }

        // Synchronous Pointer Hit-Test (Zero Latency, Zero Ghost-Drag, Includes External Stencil Captions!)
        const hitId = this.hitTestSnapshotSync(ptIn[0], ptIn[1], false, true);
        if (hitId > 0) {
          if (ev.shiftKey) {
            if (this.selectedShapeIds.includes(hitId)) {
              this.selectedShapeIds = this.selectedShapeIds.filter((i) => i !== hitId);
            } else {
              this.selectedShapeIds = [...this.selectedShapeIds, hitId];
            }
            this.callEngine('renderOffscreenFrame', { selectedIds: this.selectedShapeIds }).catch(() => {});
            this.render();
            return;
          }

          if (!this.selectedShapeIds.includes(hitId)) {
            this.setSingleSelection(hitId);
          }

          const sh = this.snapshot?.shapes.find((s) => s.id === hitId);
          if (sh && sh.kind !== 'Connector1D') {
            this.latestSmartGuides = [];
            this.latestSnappedPin = null;
            this.dragState = {
              mode: 'move',
              startPageIn: ptIn,
              curPageIn: ptIn,
              shapeId: hitId,
              origPin: [sh.pinX, sh.pinY],
              origSize: [sh.width, sh.height],
            };
          } else {
            this.lastCanvasClick = {
              shapeId: hitId,
              timeMs: performance.now(),
              clientX: ev.clientX,
              clientY: ev.clientY,
            };
            this.dragState = null;
            this.render();
          }
        } else {
          this.lastCanvasClick = null;
          // Empty canvas click/drag initiates Marquee Lasso (and deselects on simple click!)
          this.dragState = {
            mode: 'lasso',
            startPageIn: ptIn,
            curPageIn: ptIn,
          };
        }
      });

      host.addEventListener('pointermove', (ev) => {
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);
        this.cursorCoordsIn = ptIn;
        const coordEl = document.getElementById('coord-readout');
        if (coordEl) {
          coordEl.textContent = `X: ${this.formatLengthFromIn(ptIn[0])}  Y: ${this.formatLengthFromIn(ptIn[1])}`;
        }
        if (this.snapshot) {
          this.collabSession?.broadcastCursor(
            ptIn[0],
            ptIn[1],
            this.snapshot.activePageId,
            this.selectedShapeIds
          );
        }

        // Guard against any stray drag if mouse button is already up!
        if (this.dragState && ev.buttons === 0) {
          cancelActiveGesture();
          return;
        }

        if (!this.dragState) {
          if (this.snapshot && this.activeTool === 'pointer') {
            const hov = this.hitTestSnapshotSync(ptIn[0], ptIn[1], true) || null;
            if (hov !== this.hoveredShapeId) {
              this.hoveredShapeId = hov;
              const portLayer = document.getElementById('port-overlay-layer');
              if (portLayer) {
                portLayer.innerHTML = this.renderPortsHtml(this.snapshot, 96);
              }
            }
          }
          return;
        }

        if (this.dragState.mode === 'pan') {
          const vp = document.getElementById('canvas-viewport');
          if (vp && this.dragState.panStartClient && this.dragState.panStartScroll) {
            vp.scrollLeft = this.dragState.panStartScroll[0] - (ev.clientX - this.dragState.panStartClient[0]);
            vp.scrollTop = this.dragState.panStartScroll[1] - (ev.clientY - this.dragState.panStartClient[1]);
            this.syncViewportRulersAndMinimap();
          }
          return;
        }

        if (!this.snapshot) return;
        this.dragState.curPageIn = ptIn;
        const overlay = document.getElementById('gesture-overlay-svg');
        if (!overlay) return;

        const ppi = 96;
        const pageH = this.snapshot.pageHeightIn;
        const sx = (x: number) => x * ppi;
        const sy = (y: number) => (pageH - y) * ppi;

        if (this.dragState.mode === 'ink' && this.dragState.inkPoints) {
          const coalesced = ev.getCoalescedEvents ? ev.getCoalescedEvents() : [ev];
          for (const ce of coalesced) {
            const p = this.clientToPageInches(host, ce.clientX, ce.clientY);
            this.dragState.inkPoints.push({
              x: p[0],
              y: p[1],
              pressure: ce.pressure || 0.5,
              tilt_x: ce.tiltX || 0,
              tilt_y: ce.tiltY || 0,
            });
          }
          const ptsAttr = this.dragState.inkPoints.map((p) => `${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`).join(' ');
          const strokeCol = this.activeTool === 'highlighter' ? '#fde047' : '#2563eb';
          const sw = this.activeTool === 'highlighter' ? 14 : 3;
          const op = this.activeTool === 'highlighter' ? 0.45 : 1.0;
          overlay.innerHTML = `<polyline points="${ptsAttr}" fill="none" stroke="${strokeCol}" stroke-width="${sw}" stroke-opacity="${op}" stroke-linecap="round"/>`;
        } else if (this.dragState.mode === 'freeform' && this.dragState.freeformPoints) {
          this.dragState.freeformPoints.push(ptIn);
          const ptsAttr = this.dragState.freeformPoints
            .map((p) => `${sx(p[0]).toFixed(1)},${sy(p[1]).toFixed(1)}`)
            .join(' ');
          overlay.innerHTML = `<polyline points="${ptsAttr}" fill="none" stroke="#7c3aed" stroke-width="2.5" stroke-dasharray="4,2" stroke-linecap="round"/>`;
        } else if (this.dragState.mode === 'line') {
          overlay.innerHTML = `<line x1="${sx(this.dragState.startPageIn[0])}" y1="${sy(this.dragState.startPageIn[1])}" x2="${sx(ptIn[0])}" y2="${sy(ptIn[1])}" stroke="#1e293b" stroke-width="2.2" stroke-dasharray="5,3"/>`;
        } else if (this.dragState.mode === 'arc') {
          const x1 = sx(this.dragState.startPageIn[0]);
          const y1 = sy(this.dragState.startPageIn[1]);
          const x2 = sx(ptIn[0]);
          const y2 = sy(ptIn[1]);
          const mx = (x1 + x2) * 0.5 - (y2 - y1) * 0.28;
          const my = (y1 + y2) * 0.5 + (x2 - x1) * 0.28;
          overlay.innerHTML = `<path d="M ${x1.toFixed(1)} ${y1.toFixed(1)} Q ${mx.toFixed(1)} ${my.toFixed(1)} ${x2.toFixed(1)} ${y2.toFixed(1)}" fill="none" stroke="#2563eb" stroke-width="2.2" stroke-dasharray="5,3"/>`;
        } else if (this.dragState.mode === 'conn_segment') {
          const axis = this.dragState.segAxis ?? 'x';
          if (axis === 'x') {
            overlay.innerHTML = `<line x1="${sx(ptIn[0]).toFixed(1)}" y1="0" x2="${sx(ptIn[0]).toFixed(1)}" y2="${sy(0).toFixed(1)}" stroke="#0284c7" stroke-width="2" stroke-dasharray="5,3"/><circle cx="${sx(ptIn[0]).toFixed(1)}" cy="${sy(ptIn[1]).toFixed(1)}" r="6" fill="#38bdf8" stroke="#0369a1" stroke-width="2"/>`;
          } else {
            overlay.innerHTML = `<line x1="0" y1="${sy(ptIn[1]).toFixed(1)}" x2="${sx(this.snapshot.pageWidthIn).toFixed(1)}" y2="${sy(ptIn[1]).toFixed(1)}" stroke="#0284c7" stroke-width="2" stroke-dasharray="5,3"/><circle cx="${sx(ptIn[0]).toFixed(1)}" cy="${sy(ptIn[1]).toFixed(1)}" r="6" fill="#38bdf8" stroke="#0369a1" stroke-width="2"/>`;
          }
        } else if (this.dragState.mode === 'conn_endpoint') {
          const nearPort = this.findNearestPortOnPage(ptIn[0], ptIn[1], this.dragState.shapeId ?? 0);
          const drawPt: [number, number] = nearPort ? [nearPort.xIn, nearPort.yIn] : ptIn;
          const snapRing = nearPort
            ? `<circle cx="${sx(nearPort.xIn).toFixed(1)}" cy="${sy(nearPort.yIn).toFixed(1)}" r="11" fill="rgba(22,163,74,0.16)" stroke="#16a34a" stroke-width="2.2"/>`
            : '';
          overlay.innerHTML = `${snapRing}<circle cx="${sx(drawPt[0]).toFixed(1)}" cy="${sy(drawPt[1]).toFixed(1)}" r="6.5" fill="#10b981" stroke="#111827" stroke-width="2"/>`;
        } else if (this.dragState.mode === 'control' || this.dragState.mode === 'vertex') {
          const fill = this.dragState.mode === 'control' ? '#facc15' : '#22d3ee';
          overlay.innerHTML = `<circle cx="${sx(ptIn[0]).toFixed(1)}" cy="${sy(ptIn[1]).toFixed(1)}" r="7" fill="${fill}" stroke="#111827" stroke-width="2"/>`;
        } else if (this.dragState.mode === 'primitive' || this.dragState.mode === 'lasso') {
          const x0 = Math.min(sx(this.dragState.startPageIn[0]), sx(ptIn[0]));
          const y0 = Math.min(sy(this.dragState.startPageIn[1]), sy(ptIn[1]));
          const rw = Math.abs(sx(ptIn[0]) - sx(this.dragState.startPageIn[0]));
          const rh = Math.abs(sy(ptIn[1]) - sy(this.dragState.startPageIn[1]));
          const stroke = this.dragState.mode === 'lasso' ? '#0284c7' : '#2563eb';
          const fill = this.dragState.mode === 'lasso' ? 'rgba(2,132,199,0.10)' : 'rgba(37,99,235,0.12)';
          const kind = this.dragState.primitiveKind;
          if (kind === 'ellipse') {
            overlay.innerHTML = `<ellipse cx="${(x0 + rw * 0.5).toFixed(1)}" cy="${(y0 + rh * 0.5).toFixed(1)}" rx="${(rw * 0.5).toFixed(1)}" ry="${(rh * 0.5).toFixed(1)}" fill="${fill}" stroke="${stroke}" stroke-dasharray="5,4" stroke-width="1.8"/>`;
          } else if (kind === 'diamond') {
            overlay.innerHTML = `<polygon points="${(x0 + rw * 0.5).toFixed(1)},${y0.toFixed(1)} ${(x0 + rw).toFixed(1)},${(y0 + rh * 0.5).toFixed(1)} ${(x0 + rw * 0.5).toFixed(1)},${(y0 + rh).toFixed(1)} ${x0.toFixed(1)},${(y0 + rh * 0.5).toFixed(1)}" fill="${fill}" stroke="${stroke}" stroke-dasharray="5,4" stroke-width="1.8"/>`;
          } else if (kind === 'triangle') {
            overlay.innerHTML = `<polygon points="${(x0 + rw * 0.5).toFixed(1)},${y0.toFixed(1)} ${(x0 + rw).toFixed(1)},${(y0 + rh).toFixed(1)} ${x0.toFixed(1)},${(y0 + rh).toFixed(1)}" fill="${fill}" stroke="${stroke}" stroke-dasharray="5,4" stroke-width="1.8"/>`;
          } else if (kind === 'hexagon') {
            overlay.innerHTML = `<polygon points="${(x0 + rw * 0.25).toFixed(1)},${y0.toFixed(1)} ${(x0 + rw * 0.75).toFixed(1)},${y0.toFixed(1)} ${(x0 + rw).toFixed(1)},${(y0 + rh * 0.5).toFixed(1)} ${(x0 + rw * 0.75).toFixed(1)},${(y0 + rh).toFixed(1)} ${(x0 + rw * 0.25).toFixed(1)},${(y0 + rh).toFixed(1)} ${x0.toFixed(1)},${(y0 + rh * 0.5).toFixed(1)}" fill="${fill}" stroke="${stroke}" stroke-dasharray="5,4" stroke-width="1.8"/>`;
          } else if (kind === 'star') {
            overlay.innerHTML = `<polygon points="${(x0 + rw * 0.5).toFixed(1)},${y0.toFixed(1)} ${(x0 + rw * 0.62).toFixed(1)},${(y0 + rh * 0.36).toFixed(1)} ${(x0 + rw).toFixed(1)},${(y0 + rh * 0.38).toFixed(1)} ${(x0 + rw * 0.7).toFixed(1)},${(y0 + rh * 0.62).toFixed(1)} ${(x0 + rw * 0.81).toFixed(1)},${(y0 + rh).toFixed(1)} ${(x0 + rw * 0.5).toFixed(1)},${(y0 + rh * 0.78).toFixed(1)} ${(x0 + rw * 0.19).toFixed(1)},${(y0 + rh).toFixed(1)} ${(x0 + rw * 0.3).toFixed(1)},${(y0 + rh * 0.62).toFixed(1)} ${x0.toFixed(1)},${(y0 + rh * 0.38).toFixed(1)} ${(x0 + rw * 0.38).toFixed(1)},${(y0 + rh * 0.36).toFixed(1)}" fill="${fill}" stroke="${stroke}" stroke-dasharray="5,4" stroke-width="1.8"/>`;
          } else {
            const rx = kind === 'round_rect' ? 14 : 0;
            overlay.innerHTML = `<rect x="${x0}" y="${y0}" width="${rw}" height="${rh}" rx="${rx}" fill="${fill}" stroke="${stroke}" stroke-dasharray="5,4" stroke-width="1.8"/>`;
          }
        } else if (this.dragState.mode === 'connect') {
          const nearPort = this.findNearestPortOnPage(ptIn[0], ptIn[1], this.dragState.shapeId ?? 0);
          const target2D = nearPort ? nearPort.shapeId : this.hitTestSnapshotSync(ptIn[0], ptIn[1], true);
          let targetRing = '';
          if (nearPort) {
            targetRing = `<circle cx="${sx(nearPort.xIn).toFixed(1)}" cy="${sy(nearPort.yIn).toFixed(1)}" r="10" fill="rgba(22,163,74,0.18)" stroke="#16a34a" stroke-width="2.4"/>`;
          } else if (target2D > 0) {
            const tsh = this.snapshot.shapes.find((s) => s.id === target2D);
            if (tsh) {
              targetRing = `<rect x="${sx(tsh.pinX - tsh.width * 0.5) - 3}" y="${sy(tsh.pinY + tsh.height * 0.5) - 3}" width="${tsh.width * ppi + 6}" height="${tsh.height * ppi + 6}" rx="4" fill="none" stroke="#16a34a" stroke-width="2.2"/>`;
            }
          }
          const endPt: [number, number] = nearPort ? [nearPort.xIn, nearPort.yIn] : ptIn;
          const lineStart = this.dragState.origPin ?? this.dragState.startPageIn;
          overlay.innerHTML = `${targetRing}<line x1="${sx(lineStart[0])}" y1="${sy(lineStart[1])}" x2="${sx(endPt[0])}" y2="${sy(endPt[1])}" stroke="#10b981" stroke-width="2.5" stroke-dasharray="6,4"/>`;
        } else if (this.dragState.mode === 'rotate' && this.dragState.origPin && this.dragState.origSize) {
          const [cx, cy] = this.dragState.origPin;
          const [w, h] = this.dragState.origSize;
          const rad = Math.atan2(ptIn[1] - cy, ptIn[0] - cx) - Math.PI * 0.5;
          const deg = (rad * 180) / Math.PI;
          overlay.innerHTML = `
            <g transform="rotate(${-deg.toFixed(1)} ${sx(cx).toFixed(1)} ${sy(cy).toFixed(1)})">
              <rect x="${sx(cx - w * 0.5)}" y="${sy(cy + h * 0.5)}" width="${w * ppi}" height="${h * ppi}" fill="rgba(29,78,216,0.10)" stroke="#1d4ed8" stroke-dasharray="4,3" stroke-width="2"/>
            </g>
          `;
        } else if (
          this.dragState.mode === 'resize' &&
          this.dragState.origPin &&
          this.dragState.origSize &&
          this.dragState.resizeHandle
        ) {
          const next = this.computeResizedBox(
            this.dragState.origPin,
            this.dragState.origSize,
            this.dragState.resizeHandle,
            ptIn[0] - this.dragState.startPageIn[0],
            ptIn[1] - this.dragState.startPageIn[1],
            ev.shiftKey
          );
          const rx = sx(next.pinX - next.width * 0.5);
          const ry = sy(next.pinY + next.height * 0.5);
          const rw = next.width * ppi;
          const rh = next.height * ppi;
          const tipX = rx + rw * 0.5;
          const tipY = Math.max(18, ry - 14);
          overlay.innerHTML = `
            <rect x="${rx.toFixed(1)}" y="${ry.toFixed(1)}" width="${rw.toFixed(1)}" height="${rh.toFixed(1)}" fill="rgba(37,99,235,0.10)" stroke="#2563eb" stroke-dasharray="4,3" stroke-width="2"/>
            <rect x="${(tipX - 52).toFixed(1)}" y="${(tipY - 11).toFixed(1)}" width="104" height="20" rx="5" fill="#0f172a" fill-opacity="0.88"/>
            <text x="${tipX.toFixed(1)}" y="${(tipY + 3).toFixed(1)}" font-family="Segoe UI, Roboto, sans-serif" font-size="11" font-weight="600" fill="#ffffff" text-anchor="middle">${next.width.toFixed(2)}" × ${next.height.toFixed(2)}"</text>
          `;
        } else if (this.dragState.mode === 'move' && this.dragState.origPin && this.dragState.shapeId) {
          const dx = ptIn[0] - this.dragState.startPageIn[0];
          const dy = ptIn[1] - this.dragState.startPageIn[1];
          if (Math.hypot(dx, dy) < 0.04) return;
          const sh = this.snapshot.shapes.find((s) => s.id === this.dragState?.shapeId);
          if (sh) {
            const candX = this.dragState.origPin[0] + dx;
            const candY = this.dragState.origPin[1] + dy;
            const useGuides = this.showSmartGuides && !ev.altKey;

            if (!useGuides) {
              this.latestSmartGuides = [];
              this.latestSnappedPin = null;
            } else if (!this.smartGuideReqInFlight && this.selectedShapeIds.length <= 1) {
              this.smartGuideReqInFlight = true;
              this.callEngine('computeSmartGuides', {
                shapeId: sh.id,
                candX,
                candY,
                width: sh.width,
                height: sh.height,
              })
                .then((gRes) => {
                  this.latestSmartGuides = gRes.guides ?? [];
                  this.latestSnappedPin = [gRes.snappedX ?? candX, gRes.snappedY ?? candY];
                })
                .finally(() => {
                  this.smartGuideReqInFlight = false;
                });
            }

            const drawX = useGuides && this.latestSnappedPin ? this.latestSnappedPin[0] : candX;
            const drawY = useGuides && this.latestSnappedPin ? this.latestSnappedPin[1] : candY;
            const guideSvg = useGuides
              ? this.latestSmartGuides
                  .map((g) => {
                    if (g.kind === 'equal_spacing') {
                      if (g.axis === 'x') {
                        const x = sx(g.posIn);
                        const y1 = sy(g.toIn);
                        const y2 = sy(g.fromIn);
                        return `<line x1="${x}" y1="${y1}" x2="${x}" y2="${y2}" stroke="#f97316" stroke-width="1.8"/><line x1="${x - 5}" y1="${y1}" x2="${x + 5}" y2="${y1}" stroke="#f97316" stroke-width="1.8"/><line x1="${x - 5}" y1="${y2}" x2="${x + 5}" y2="${y2}" stroke="#f97316" stroke-width="1.8"/>`;
                      }
                      const y = sy(g.posIn);
                      const x1 = sx(g.fromIn);
                      const x2 = sx(g.toIn);
                      return `<line x1="${x1}" y1="${y}" x2="${x2}" y2="${y}" stroke="#f97316" stroke-width="1.8"/><line x1="${x1}" y1="${y - 5}" x2="${x1}" y2="${y + 5}" stroke="#f97316" stroke-width="1.8"/><line x1="${x2}" y1="${y - 5}" x2="${x2}" y2="${y + 5}" stroke="#f97316" stroke-width="1.8"/>`;
                    }
                    const col = g.kind === 'center' ? '#ec4899' : '#06b6d4';
                    return g.axis === 'x'
                      ? `<line x1="${sx(g.posIn)}" y1="${sy(g.toIn)}" x2="${sx(g.posIn)}" y2="${sy(g.fromIn)}" stroke="${col}" stroke-width="1.6" stroke-dasharray="4,2"/>`
                      : `<line x1="${sx(g.fromIn)}" y1="${sy(g.posIn)}" x2="${sx(g.toIn)}" y2="${sy(g.posIn)}" stroke="${col}" stroke-width="1.6" stroke-dasharray="4,2"/>`;
                  })
                  .join('')
              : '';

            // Container / Swimlane emerald drop-target glow + Container member ghost outlines
            let containerGlowSvg = '';
            if (!sh.isSwimlane) {
              const movingSet = new Set(this.selectedShapeIds);
              let bestTarget: { id: number; area: number; x0: number; y0: number; w: number; h: number } | null = null;
              for (const cand of this.snapshot.shapes) {
                if (movingSet.has(cand.id) || (!cand.isContainer && !cand.isSwimlane)) continue;
                const x0 = cand.pinX - cand.width * 0.5;
                const x1 = cand.pinX + cand.width * 0.5;
                const y0 = cand.pinY - cand.height * 0.5;
                const y1 = cand.pinY + cand.height * 0.5;
                if (drawX >= x0 - 0.14 && drawX <= x1 + 0.14 && drawY >= y0 - 0.14 && drawY <= y1 + 0.14) {
                  const area = Math.max(0.1, cand.width) * Math.max(0.1, cand.height);
                  if (!bestTarget || area < bestTarget.area) {
                    bestTarget = { id: cand.id, area, x0, y0: y1, w: cand.width, h: cand.height };
                  }
                }
              }
              if (bestTarget) {
                const gx = sx(bestTarget.x0) - 3;
                const gy = sy(bestTarget.y0) - 3;
                const gw = bestTarget.w * ppi + 6;
                const gh = bestTarget.h * ppi + 6;
                containerGlowSvg = `<rect x="${gx.toFixed(1)}" y="${gy.toFixed(1)}" width="${gw.toFixed(1)}" height="${gh.toFixed(1)}" rx="8" fill="rgba(16,185,129,0.10)" stroke="#059669" stroke-width="3"/>`;
              }
            }

            let memberPreviewSvg = '';
            if (sh.isContainer && sh.containerMembers && sh.containerMembers.length > 0) {
              const effDx = drawX - this.dragState.origPin[0];
              const effDy = drawY - this.dragState.origPin[1];
              for (const mid of sh.containerMembers) {
                const msh = this.snapshot.shapes.find((s) => s.id === mid);
                if (!msh) continue;
                memberPreviewSvg += `<rect x="${sx(msh.pinX + effDx - msh.width * 0.5)}" y="${sy(msh.pinY + effDy + msh.height * 0.5)}" width="${msh.width * ppi}" height="${msh.height * ppi}" fill="rgba(37,99,235,0.06)" stroke="#3b82f6" stroke-dasharray="3,3" stroke-width="1.4"/>`;
              }
            }

            overlay.innerHTML = `
              ${containerGlowSvg}
              ${guideSvg}
              <rect x="${sx(drawX - sh.width * 0.5)}" y="${sy(drawY + sh.height * 0.5)}" width="${sh.width * ppi}" height="${sh.height * ppi}" fill="rgba(37,99,235,0.1)" stroke="#2563eb" stroke-dasharray="4,3" stroke-width="2"/>
              ${memberPreviewSvg}
            `;
          }
        }
      });

      host.addEventListener('pointerup', async (ev) => {
        if (!this.dragState) return;
        const st = this.dragState;
        this.dragState = null;
        host.classList.remove('panning');
        const overlay = document.getElementById('gesture-overlay-svg');
        if (overlay) overlay.innerHTML = '';
        const ptIn = this.clientToPageInches(host, ev.clientX, ev.clientY);

        if (st.mode === 'pan') {
          return;
        } else if (st.mode === 'open_text_edit' && st.shapeId) {
          this.openInlineShapeTextEditor(st.shapeId);
          return;
        } else if (st.mode === 'ink' && st.inkPoints && st.inkPoints.length >= 2) {
          const isHi = this.activeTool === 'highlighter';
          const res = await this.callEngine('addInkStroke', {
            colorHex: isHi ? '#FACC15' : '#2563EB',
            widthPt: isHi ? 12 : 2.25,
            highlighter: isHi,
            points: st.inkPoints,
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.render();
        } else if (st.mode === 'freeform' && st.freeformPoints && st.freeformPoints.length >= 2) {
          const start = st.freeformPoints[0];
          const closed = Math.hypot(ptIn[0] - start[0], ptIn[1] - start[1]) < 0.28 && st.freeformPoints.length >= 5;
          const res = await this.callEngine('drawFreeformCurve', {
            points: st.freeformPoints,
            closed,
          });
          if (res.shapeId > 0) {
            this.snapshot = res.snapshot;
            this.setSingleSelection(res.shapeId);
            this.activeTool = 'pointer';
            this.persistMutationSideEffects(res.wal);
          }
          this.render();
        } else if (st.mode === 'line') {
          let [x2, y2] = ptIn;
          if (Math.hypot(x2 - st.startPageIn[0], y2 - st.startPageIn[1]) < 0.08) {
            x2 = st.startPageIn[0] + 1.8;
            y2 = st.startPageIn[1];
          }
          const res = await this.callEngine('drawLine', {
            x1: st.startPageIn[0],
            y1: st.startPageIn[1],
            x2,
            y2,
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.setSingleSelection(res.shapeId);
          this.activeTool = 'pointer';
          this.render();
        } else if (st.mode === 'arc') {
          let [x2, y2] = ptIn;
          if (Math.hypot(x2 - st.startPageIn[0], y2 - st.startPageIn[1]) < 0.08) {
            x2 = st.startPageIn[0] + 1.8;
            y2 = st.startPageIn[1] + 0.6;
          }
          const chord = Math.hypot(x2 - st.startPageIn[0], y2 - st.startPageIn[1]);
          const res = await this.callEngine('drawArc', {
            x1: st.startPageIn[0],
            y1: st.startPageIn[1],
            x2,
            y2,
            bowIn: Math.max(0.2, chord * 0.25),
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.setSingleSelection(res.shapeId);
          this.activeTool = 'pointer';
          this.render();
        } else if (st.mode === 'primitive') {
          const kind = st.primitiveKind ?? 'rect';
          let x1 = st.startPageIn[0];
          let y1 = st.startPageIn[1];
          let x2 = ptIn[0];
          let y2 = ptIn[1];
          if (Math.hypot(x2 - x1, y2 - y1) < 0.08) {
            const isSym =
              kind === 'ellipse' ||
              kind === 'diamond' ||
              kind === 'triangle' ||
              kind === 'hexagon' ||
              kind === 'star';
            const hw = isSym ? 0.68 : 0.85;
            const hh = kind === 'text' ? 0.28 : isSym ? 0.68 : 0.5;
            x1 -= hw;
            x2 = st.startPageIn[0] + hw;
            y1 -= hh;
            y2 = st.startPageIn[1] + hh;
          }
          const res = await this.callEngine('drawPrimitive', { kind, x1, y1, x2, y2 });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.setSingleSelection(res.shapeId);
          this.activeTool = 'pointer';
          if (kind === 'text' && res.shapeId > 0) {
            const clearRes = await this.callEngine('setShapeText', { shapeId: res.shapeId, text: '' });
            if (clearRes?.snapshot) this.snapshot = clearRes.snapshot;
            this.editingShapeId = res.shapeId;
            this.inlineTextDraft = '';
          }
          this.render();
        } else if (st.mode === 'conn_segment' && st.shapeId) {
          const coordIn = st.segAxis === 'y' ? ptIn[1] : ptIn[0];
          const res = await this.callEngine('dragConnectorSegment', {
            shapeId: st.shapeId,
            segIndex: st.segIndex ?? 1,
            coordIn,
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.render();
        } else if (st.mode === 'conn_endpoint' && st.shapeId) {
          const nearPort = !ev.altKey ? this.findNearestPortOnPage(ptIn[0], ptIn[1], st.shapeId) : null;
          const target2D = nearPort ? nearPort.shapeId : this.hitTestSnapshotSync(ptIn[0], ptIn[1], true);
          const res = await this.callEngine('moveConnectorEndpointWithPort', {
            shapeId: st.shapeId,
            isBegin: Boolean(st.connIsBegin),
            xIn: nearPort ? nearPort.xIn : ptIn[0],
            yIn: nearPort ? nearPort.yIn : ptIn[1],
            targetShapeId: target2D,
            targetPortCell: nearPort ? nearPort.portCell : '',
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.render();
        } else if (st.mode === 'connect') {
          const nearPort = !ev.altKey ? this.findNearestPortOnPage(ptIn[0], ptIn[1], st.shapeId ?? 0) : null;
          const to2D = nearPort ? nearPort.shapeId : this.hitTestSnapshotSync(ptIn[0], ptIn[1], true);
          const fromId = st.shapeId ?? 0;
          const dist = Math.hypot(ptIn[0] - st.startPageIn[0], ptIn[1] - st.startPageIn[1]);
          const portOrigin = st.origPin ?? st.startPageIn;
          if (st.autoConnectDir && dist <= 0.12 && fromId > 0) {
            // One-click Visio AutoConnect: stamp & wire next shape in `autoConnectDir` (or link aligned neighbor)!
            const acRes = await this.callEngine('autoConnect', {
              sourceShapeId: fromId,
              dir: st.autoConnectDir,
            });
            this.persistMutationSideEffects(acRes.wal);
            this.snapshot = acRes.snapshot;
            if (acRes.targetShapeId > 0) {
              this.setSingleSelection(acRes.targetShapeId);
            }
          } else if (st.autoConnectDir && dist > 0.12 && fromId > 0 && (to2D === 0 || to2D === fromId)) {
            // Dragged AutoConnect onto empty canvas -> stamp a connected Process step at drop location!
            const pRes = await this.callEngine('drawPrimitive', {
              kind: 'round_rect',
              x1: ptIn[0] - 0.8,
              y1: ptIn[1] - 0.45,
              x2: ptIn[0] + 0.8,
              y2: ptIn[1] + 0.45,
            });
            if (pRes.shapeId > 0) {
              const cRes = await this.callEngine('connectPointsOrShapes', {
                x1: portOrigin[0],
                y1: portOrigin[1],
                x2: ptIn[0],
                y2: ptIn[1],
                fromId,
                toId: pRes.shapeId,
              });
              this.persistMutationSideEffects(cRes.wal);
              this.snapshot = cRes.snapshot;
              this.setSingleSelection(pRes.shapeId);
            }
          } else if ((fromId > 0 && to2D > 0 && fromId !== to2D) || dist > 0.12) {
            const res = await this.callEngine('connectPointsOrShapes', {
              x1: portOrigin[0],
              y1: portOrigin[1],
              x2: nearPort ? nearPort.xIn : ptIn[0],
              y2: nearPort ? nearPort.yIn : ptIn[1],
              fromId,
              toId: to2D === fromId ? 0 : to2D,
            });
            let finalRes = res;
            if (res.connId > 0 && nearPort && to2D !== fromId) {
              finalRes = await this.callEngine('moveConnectorEndpointWithPort', {
                shapeId: res.connId,
                isBegin: false,
                xIn: nearPort.xIn,
                yIn: nearPort.yIn,
                targetShapeId: nearPort.shapeId,
                targetPortCell: nearPort.portCell,
              });
            }
            this.persistMutationSideEffects(finalRes.wal);
            this.snapshot = finalRes.snapshot;
            if (res.connId > 0) {
              this.setSingleSelection(res.connId);
            }
          }
          this.activeTool = 'pointer';
          this.render();
        } else if (st.mode === 'control' && st.shapeId) {
          const res = await this.callEngine('moveControlHandle', {
            shapeId: st.shapeId,
            rowName: st.controlRowName ?? 'Row_1',
            xIn: ptIn[0],
            yIn: ptIn[1],
          });
          this.snapshot = res.snapshot;
          this.persistMutationSideEffects(res.wal);
          this.render();
        } else if (st.mode === 'vertex' && st.shapeId !== undefined) {
          const res = await this.callEngine('moveShapeVertex', {
            shapeId: st.shapeId,
            geomIndex: st.geomIndex ?? 0,
            rowIx: st.vertexRowIx ?? 1,
            xIn: ptIn[0],
            yIn: ptIn[1],
          });
          this.activeShapeVertices = res.vertices ?? [];
          this.snapshot = res.snapshot;
          this.persistMutationSideEffects(res.wal);
          this.render();
        } else if (st.mode === 'lasso') {
          const dx = Math.abs(ptIn[0] - st.startPageIn[0]);
          const dy = Math.abs(ptIn[1] - st.startPageIn[1]);
          if (dx > 0.08 || dy > 0.08) {
            const qRes = await this.callEngine('queryBoxShapes', {
              x1: st.startPageIn[0],
              y1: st.startPageIn[1],
              x2: ptIn[0],
              y2: ptIn[1],
            });
            this.selectedShapeIds = qRes.ids ?? [];
          } else {
            this.setSingleSelection(null);
            await this.callEngine('renderOffscreenFrame', { selectedIds: [] });
          }
          this.render();
        } else if (st.mode === 'rotate' && st.shapeId && st.origPin && st.origSize) {
          const [cx, cy] = st.origPin;
          const rad = Math.atan2(ptIn[1] - cy, ptIn[0] - cx) - Math.PI * 0.5;
          const angleDeg = Math.round(((rad * 180) / Math.PI) / 5) * 5;
          const res = await this.callEngine('transformShape', {
            shapeId: st.shapeId,
            pinX: cx,
            pinY: cy,
            width: st.origSize[0],
            height: st.origSize[1],
            angleDeg,
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.render();
        } else if (st.mode === 'resize' && st.shapeId && st.origPin && st.origSize && st.resizeHandle) {
          const next = this.computeResizedBox(
            st.origPin,
            st.origSize,
            st.resizeHandle,
            ptIn[0] - st.startPageIn[0],
            ptIn[1] - st.startPageIn[1],
            ev.shiftKey
          );
          const res = await this.callEngine('transformShape', {
            shapeId: st.shapeId,
            pinX: next.pinX,
            pinY: next.pinY,
            width: next.width,
            height: next.height,
            angleDeg: st.origAngleDeg ?? 0,
          });
          this.persistMutationSideEffects(res.wal);
          this.snapshot = res.snapshot;
          this.render();
        } else if (st.mode === 'move' && st.shapeId && st.origPin) {
          const dx = ptIn[0] - st.startPageIn[0];
          const dy = ptIn[1] - st.startPageIn[1];
          if (Math.hypot(dx, dy) > 0.04) {
            this.lastCanvasClick = null;
            if (this.selectedShapeIds.length > 1 && this.selectedShapeIds.includes(st.shapeId)) {
              const res = await this.callEngine('moveShapesDelta', {
                shapeIds: this.selectedShapeIds,
                dxIn: dx,
                dyIn: dy,
              });
              this.persistMutationSideEffects(res.wal);
              this.snapshot = res.snapshot;
            } else {
              const useSnap = this.showSmartGuides && !ev.altKey && this.latestSnappedPin;
              const finalX = useSnap ? this.latestSnappedPin![0] : st.origPin[0] + dx;
              const finalY = useSnap ? this.latestSnappedPin![1] : st.origPin[1] + dy;
              const res = await this.callEngine('moveShape', {
                shapeId: st.shapeId,
                xIn: finalX,
                yIn: finalY,
              });
              this.persistMutationSideEffects(res.wal);
              this.snapshot = res.snapshot;
            }
          } else {
            this.lastCanvasClick = {
              shapeId: st.shapeId,
              timeMs: performance.now(),
              clientX: ev.clientX,
              clientY: ev.clientY,
            };
            this.callEngine('renderOffscreenFrame', { selectedIds: this.selectedShapeIds }).catch(() => {});
          }
          this.latestSmartGuides = [];
          this.latestSnappedPin = null;
          this.render();
        }
      });
    }

    // Inspector & Layer & Comment Listeners
    document.getElementById('shape-selector')?.addEventListener('change', async (e) => {
      this.setSingleSelection(Number((e.target as HTMLSelectElement).value));
      await this.callEngine('renderOffscreenFrame', { selectedIds: this.selectedShapeIds });
      this.render();
    });

    document.getElementById('shape-text-input')?.addEventListener('change', async (e) => {
      if (this.primarySelectedId === null) return;
      const res = await this.callEngine('setShapeText', {
        shapeId: this.primarySelectedId,
        text: (e.target as HTMLInputElement).value,
      });
      this.persistMutationSideEffects(res.wal);
      this.snapshot = res.snapshot;
      this.render();
    });

    document.querySelectorAll('.quick-dim').forEach((inp) => {
      const applyDim = async (el: HTMLInputElement) => {
        if (!this.primarySelectedId) return;
        const cellName = el.dataset.cell ?? 'Width';
        const raw = el.value.trim();
        const formula =
          /^-?\d+(\.\d+)?$/.test(raw)
            ? `${raw} ${cellName === 'Angle' ? 'deg' : 'in'}`
            : raw;
        const res = await this.callEngine('setCellFormula', {
          shapeId: this.primarySelectedId,
          cellName,
          formula,
          forceSetf: false,
        });
        if (res.applied === false) {
          this.showToast(`Cell "${cellName}" is protected by GUARD(). Use fx bar + SETF to override.`, 'warn');
        } else {
          this.persistMutationSideEffects(res.wal);
        }
        this.snapshot = res.snapshot;
        this.render();
      };
      inp.addEventListener('change', (e) => applyDim(e.target as HTMLInputElement));
      inp.addEventListener('keydown', (e) => {
        const ke = e as KeyboardEvent;
        if (ke.key === 'Enter') {
          ke.preventDefault();
          applyDim(e.target as HTMLInputElement);
        }
      });
    });

    document.querySelectorAll('.layer-vis-chk, .layer-lock-chk').forEach((chk) => {
      chk.addEventListener('change', async () => {
        const ix = Number((chk as HTMLElement).dataset.ix);
        const visEl = document.querySelector(`.layer-vis-chk[data-ix="${ix}"]`) as HTMLInputElement;
        const lockEl = document.querySelector(`.layer-lock-chk[data-ix="${ix}"]`) as HTMLInputElement;
        const res = await this.callEngine('updateLayer', {
          ix,
          visible: visEl?.checked ?? true,
          lock: lockEl?.checked ?? false,
          colorHex: '',
        });
        this.snapshot = res.snapshot;
        this.persistMutationSideEffects(res.wal);
        this.render();
      });
    });

    document.getElementById('btn-post-comment')?.addEventListener('click', async () => {
      const inp = document.getElementById('new-comment-input') as HTMLInputElement | null;
      if (!inp || !inp.value.trim()) return;
      const res = await this.callEngine('addComment', {
        shapeId: this.primarySelectedId ?? 0,
        author: this.cloudState.user?.email ?? this.collabSession.peerName,
        text: inp.value.trim(),
      });
      this.snapshot = res.snapshot;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    document.getElementById('btn-apply-formula')?.addEventListener('click', () => {
      this.applyFormulaFromStrip();
    });

    document.querySelectorAll('.page-tab-btn[data-page]').forEach((btn) => {
      const el = btn as HTMLElement;
      const pageId = Number(el.dataset.page);
      el.addEventListener('click', async () => {
        this.snapshot = (await this.callEngine('selectPage', { pageId })).snapshot;
        this.selectedShapeIds = [];
        this.editingShapeId = null;
        await this.syncCanvasAfterDocumentOpen();
        this.render();
      });
      el.addEventListener('contextmenu', (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        this.contextMenu = {
          kind: 'page',
          x: ev.clientX,
          y: ev.clientY,
          targetPageId: pageId,
        };
        this.render();
      });
      el.addEventListener('dblclick', (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const curName = this.snapshot?.pages.find((p) => p.id === pageId)?.name ?? `Page-${pageId}`;
        this.renamingPageId = pageId;
        this.renamePageDraft = curName;
        this.render();
      });
    });

    const pageRenameInput = document.getElementById('page-rename-inline-input') as HTMLInputElement | null;
    if (pageRenameInput) {
      const targetPid = Number(pageRenameInput.dataset.renamePage ?? this.snapshot?.activePageId ?? 1);
      let pageHandled = false;
      pageRenameInput.addEventListener('input', () => {
        this.renamePageDraft = pageRenameInput.value;
      });
      pageRenameInput.addEventListener('keydown', (ev) => {
        if (ev.key === 'Enter') {
          ev.preventDefault();
          ev.stopPropagation();
          if (!pageHandled) {
            pageHandled = true;
            void this.commitPageRename(targetPid, pageRenameInput.value);
          }
        } else if (ev.key === 'Escape') {
          ev.preventDefault();
          ev.stopPropagation();
          pageHandled = true;
          this.renamingPageId = null;
          this.render();
        }
      });
      pageRenameInput.addEventListener('blur', () => {
        if (this.isRebuildingDom || !pageRenameInput.isConnected) return;
        if (!pageHandled) {
          pageHandled = true;
          void this.commitPageRename(targetPid, pageRenameInput.value);
        }
      });
    }

    document.getElementById('btn-add-page')?.addEventListener('click', async () => {
      const nextNum = (this.snapshot?.pages.length ?? 1) + 1;
      const res = await this.callEngine('addPage', { name: `Page-${nextNum}` });
      this.snapshot = res.snapshot;
      this.selectedShapeIds = [];
      this.editingShapeId = null;
      this.persistMutationSideEffects(res.wal);
      this.render();
    });

    // Right-Click Context Menu Command Dispatcher ([data-ctx])
    document.querySelectorAll('[data-ctx]').forEach((item) => {
      item.addEventListener('click', async (ev) => {
        ev.stopPropagation();
        const action = (item as HTMLElement).dataset.ctx ?? '';
        const cm = this.contextMenu;
        this.contextMenu = null;
        const px = cm?.pageIn?.[0] ?? (this.snapshot?.pageWidthIn ?? 11) * 0.5;
        const py = cm?.pageIn?.[1] ?? (this.snapshot?.pageHeightIn ?? 8.5) * 0.5;

        switch (action) {
          case 'edit-text': {
            if (this.primarySelectedId) {
              this.openInlineShapeTextEditor(this.primarySelectedId);
            } else {
              this.render();
            }
            break;
          }
          case 'rotate-left':
            await this.rotateOrFlipSelected(-90);
            break;
          case 'rotate-right':
            await this.rotateOrFlipSelected(90);
            break;
          case 'flip-h':
            await this.rotateOrFlipSelected(0, true, false);
            break;
          case 'flip-v':
            await this.rotateOrFlipSelected(0, false, true);
            break;
          case 'cut':
            await this.cutSelectedShapesClean();
            break;
          case 'copy':
            await this.copySelectedShapes();
            this.render();
            break;
          case 'paste':
            await this.pasteClipboardAt();
            break;
          case 'paste-here':
            await this.pasteClipboardAt([px, py]);
            break;
          case 'duplicate': {
            if (this.selectedShapeIds.length > 0) {
              const res = await this.callEngine('duplicateShapes', {
                shapeIds: this.selectedShapeIds,
                dxIn: 0.35,
                dyIn: -0.35,
              });
              this.selectedShapeIds = res.createdIds ?? this.selectedShapeIds;
              this.snapshot = res.snapshot;
              this.persistMutationSideEffects(res.wal);
            }
            this.render();
            break;
          }
          case 'delete': {
            if (this.selectedShapeIds.length > 0) {
              const res = await this.callEngine('deleteShape', { shapeIds: this.selectedShapeIds });
              this.snapshot = res.snapshot;
              this.setSingleSelection(null);
              this.persistMutationSideEffects(res.wal);
            }
            this.render();
            break;
          }
          case 'bring-front':
          case 'send-back': {
            if (this.selectedShapeIds.length > 0) {
              const res = await this.callEngine('reorderShapes', {
                shapeIds: this.selectedShapeIds,
                mode: action === 'bring-front' ? 'front' : 'back',
              });
              this.snapshot = res.snapshot;
              this.persistMutationSideEffects(res.wal);
            }
            this.render();
            break;
          }
          case 'group':
            await this.groupSelectedShapes();
            break;
          case 'ungroup':
            await this.ungroupSelectedShapes();
            break;
          case 'route-ortho':
          case 'route-curved':
          case 'route-straight': {
            const formula = action === 'route-curved' ? '2' : action === 'route-straight' ? '1' : '16';
            const res = await this.callEngine('batchSetCells', {
              shapeIds: this.selectedShapeIds,
              entries: [{ cell: 'ShapeRouteStyle', formula }],
              forceSetf: true,
            });
            this.snapshot = res.snapshot;
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'arrow-end':
          case 'arrow-both':
          case 'arrow-none': {
            const entries =
              action === 'arrow-both'
                ? [
                    { cell: 'BeginArrow', formula: '4' },
                    { cell: 'EndArrow', formula: '4' },
                  ]
                : action === 'arrow-none'
                  ? [
                      { cell: 'BeginArrow', formula: '0' },
                      { cell: 'EndArrow', formula: '0' },
                    ]
                  : [
                      { cell: 'BeginArrow', formula: '0' },
                      { cell: 'EndArrow', formula: '4' },
                    ];
            const res = await this.callEngine('batchSetCells', {
              shapeIds: this.selectedShapeIds,
              entries,
              forceSetf: true,
            });
            this.snapshot = res.snapshot;
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'add-callout': {
            if (this.primarySelectedId) {
              const res = await this.callEngine('insertCallout', {
                targetShapeId: this.primarySelectedId,
                text: 'Review Note',
              });
              this.snapshot = res.snapshot;
              this.persistMutationSideEffects(res.wal);
            }
            this.render();
            break;
          }
          case 'link-subprocess': {
            if (this.primarySelectedId) {
              const res = await this.callEngine('createSubprocess', {
                shapeId: this.primarySelectedId,
                subName: '',
                navigate: true,
              });
              this.snapshot = res.snapshot;
              this.selectedShapeIds = [];
              this.persistMutationSideEffects(res.wal);
            }
            this.render();
            break;
          }
          case 'open-hyperlink':
            this.activeModal = 'link';
            this.modalOutputMessage = '';
            this.render();
            break;
          case 'show-shapesheet':
            this.showShapeSheet = true;
            this.showFormulaBar = true;
            this.render();
            break;
          case 'select-all':
            this.selectedShapeIds = (this.snapshot?.shapes ?? []).map((s) => s.id);
            await this.callEngine('renderOffscreenFrame', { selectedIds: this.selectedShapeIds });
            this.render();
            break;
          case 'drop-here-rect':
          case 'drop-here-ellipse':
          case 'drop-here-diamond':
          case 'drop-here-text': {
            const kindMap: Record<string, string> = {
              'drop-here-rect': 'rect',
              'drop-here-ellipse': 'ellipse',
              'drop-here-diamond': 'diamond',
              'drop-here-text': 'text',
            };
            const kind = kindMap[action] ?? 'rect';
            const hw = kind === 'rect' ? 0.85 : 0.65;
            const hh = kind === 'text' ? 0.28 : kind === 'rect' ? 0.5 : 0.65;
            const res = await this.callEngine('drawPrimitive', {
              kind,
              x1: px - hw,
              y1: py - hh,
              x2: px + hw,
              y2: py + hh,
            });
            this.snapshot = res.snapshot;
            this.activeTool = 'pointer';
            this.setSingleSelection(res.shapeId);
            if (kind === 'text' && res.shapeId > 0) {
              const clearRes = await this.callEngine('setShapeText', { shapeId: res.shapeId, text: '' });
              if (clearRes?.snapshot) this.snapshot = clearRes.snapshot;
              this.editingShapeId = res.shapeId;
              this.inlineTextDraft = '';
            }
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'drop-here-note': {
            const res = await this.callEngine('dropShape', {
              category: 'Callouts, Notes & Containers',
              masterName: 'Folded Note',
              xIn: px,
              yIn: py,
            });
            this.snapshot = res.snapshot;
            this.activeTool = 'pointer';
            this.setSingleSelection(res.shapeId);
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'export-jpg':
            this.render();
            await this.handleSaveInPlace('jpg');
            break;
          case 'export-pdf':
            this.render();
            await this.handleSaveInPlace('pdf');
            break;
          case 'layout-tb':
          case 'layout-lr': {
            const res = await this.callEngine('autoLayout', {
              algorithm: action === 'layout-lr' ? 'lr' : 'tb',
            });
            this.snapshot = res.snapshot;
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'zoom-fit':
            this.zoomPct = this.computeFitZoomPct();
            this.render();
            break;
          case 'toggle-grid':
            this.showGrid = !this.showGrid;
            this.render();
            break;
          case 'toggle-rulers':
            this.showRulers = !this.showRulers;
            this.render();
            break;
          case 'toggle-smart-guides':
            this.showSmartGuides = !this.showSmartGuides;
            this.latestSmartGuides = [];
            this.latestSnappedPin = null;
            this.render();
            break;
          case 'page-rename': {
            const targetPid = cm?.targetPageId ?? this.snapshot?.activePageId ?? 0;
            const curName = this.snapshot?.pages.find((p) => p.id === targetPid)?.name ?? `Page-${targetPid}`;
            this.renamingPageId = targetPid;
            this.renamePageDraft = curName;
            this.render();
            break;
          }
          case 'page-duplicate': {
            const targetPid = cm?.targetPageId ?? this.snapshot?.activePageId ?? 1;
            const res = await this.callEngine('duplicatePage', { pageId: targetPid });
            this.snapshot = res.snapshot;
            this.selectedShapeIds = [];
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'page-insert': {
            const nextNum = (this.snapshot?.pages.length ?? 1) + 1;
            const res = await this.callEngine('addPage', { name: `Page-${nextNum}` });
            this.snapshot = res.snapshot;
            this.selectedShapeIds = [];
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'page-delete': {
            const targetPid = cm?.targetPageId ?? this.snapshot?.activePageId ?? 1;
            const res = await this.callEngine('deletePage', { pageId: targetPid });
            this.snapshot = res.snapshot;
            this.selectedShapeIds = [];
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'stencil-drop-center': {
            const res = await this.callEngine('dropShape', {
              category: cm?.stencilPack ?? 'Basic Shapes',
              masterName: cm?.stencilMaster ?? 'Rectangle',
              xIn: (this.snapshot?.pageWidthIn ?? 11) * 0.5,
              yIn: (this.snapshot?.pageHeightIn ?? 8.5) * 0.5,
            });
            this.snapshot = res.snapshot;
            this.activeTool = 'pointer';
            this.setSingleSelection(res.shapeId);
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          case 'stencil-drop-three': {
            const cat = cm?.stencilPack ?? 'Basic Shapes';
            const mst = cm?.stencilMaster ?? 'Rectangle';
            const cy = (this.snapshot?.pageHeightIn ?? 8.5) * 0.5;
            const r1 = await this.callEngine('dropShape', { category: cat, masterName: mst, xIn: 3.0, yIn: cy });
            const r2 = await this.callEngine('dropShape', { category: cat, masterName: mst, xIn: 5.5, yIn: cy });
            const r3 = await this.callEngine('dropShape', { category: cat, masterName: mst, xIn: 8.0, yIn: cy });
            await this.callEngine('connectShapes', { fromId: r1.shapeId, toId: r2.shapeId });
            const res = await this.callEngine('connectShapes', { fromId: r2.shapeId, toId: r3.shapeId });
            this.snapshot = res.snapshot;
            this.activeTool = 'pointer';
            this.selectedShapeIds = [r1.shapeId, r2.shapeId, r3.shapeId];
            this.persistMutationSideEffects(res.wal);
            this.render();
            break;
          }
          default:
            this.render();
            break;
        }
      });
    });

    document.getElementById('zoom-slider')?.addEventListener('input', (e) => {
      this.zoomPct = Number((e.target as HTMLInputElement).value);
      this.render();
    });
  }

  private hitTestResizeGripSync(
    xIn: number,
    yIn: number
  ): { shapeId: number; dir: ResizeHandleDir } | null {
    if (!this.snapshot || !this.primarySelectedId) return null;
    const sh = this.snapshot.shapes.find(
      (s) => s.id === this.primarySelectedId && s.kind !== 'Connector1D'
    );
    if (!sh) return null;
    const dx = xIn - sh.pinX;
    const dy = yIn - sh.pinY;
    const rad = (-sh.angleDeg * Math.PI) / 180;
    const lx = dx * Math.cos(rad) - dy * Math.sin(rad);
    const ly = dx * Math.sin(rad) + dy * Math.cos(rad);
    const ppiZoom = 96 * (this.zoomPct / 100);
    const tolIn = Math.max(0.09, Math.min(0.22, 11 / Math.max(24, ppiZoom)));
    const hw = sh.width * 0.5;
    const hh = sh.height * 0.5;
    const handles: Array<[ResizeHandleDir, number, number]> = [
      ['nw', -hw, hh],
      ['ne', hw, hh],
      ['se', hw, -hh],
      ['sw', -hw, -hh],
      ['n', 0, hh],
      ['e', hw, 0],
      ['s', 0, -hh],
      ['w', -hw, 0],
    ];
    for (const [dir, gx, gy] of handles) {
      if (Math.hypot(lx - gx, ly - gy) <= tolIn) {
        return { shapeId: sh.id, dir };
      }
    }
    return null;
  }

  private computeResizedBox(
    origPin: [number, number],
    origSize: [number, number],
    dir: ResizeHandleDir,
    dx: number,
    dy: number,
    lockAspect = false
  ): { pinX: number; pinY: number; width: number; height: number } {
    const minDim = 0.15;
    let left = origPin[0] - origSize[0] * 0.5;
    let right = origPin[0] + origSize[0] * 0.5;
    let bottom = origPin[1] - origSize[1] * 0.5;
    let top = origPin[1] + origSize[1] * 0.5;

    if (dir.includes('w')) left = Math.min(right - minDim, left + dx);
    if (dir.includes('e')) right = Math.max(left + minDim, right + dx);
    if (dir.includes('s')) bottom = Math.min(top - minDim, bottom + dy);
    if (dir.includes('n')) top = Math.max(bottom + minDim, top + dy);

    let width = Math.max(minDim, right - left);
    let height = Math.max(minDim, top - bottom);
    if (lockAspect && origSize[1] > 1e-6 && dir.length === 2) {
      const aspect = origSize[0] / origSize[1];
      height = Math.max(minDim, width / aspect);
      if (dir.includes('n')) {
        top = bottom + height;
      } else {
        bottom = top - height;
      }
    }

    return {
      pinX: (left + right) * 0.5,
      pinY: (bottom + top) * 0.5,
      width,
      height,
    };
  }
}

new VexioChromeOsApp().boot();
