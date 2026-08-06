// Tether custom home UI: left-sidebar shell + Remote Control home page.
// Theme-aware (dark/light). Feature-complete: preserves the old home's
// functionality (ID/password + permanent password, connect types, real peers,
// online status, install prompt) inside the new layout.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_hbb/common/widgets/peer_tab_page.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart' show PeerUiType;
import 'package:flutter_hbb/common/widgets/address_book.dart';
import 'package:flutter_hbb/models/peer_tab_model.dart';
import 'package:flutter_hbb/desktop/pages/connection_page.dart' show OnlineStatusWidget;
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_tab_page.dart';

class TetherHomePage extends StatefulWidget {
  const TetherHomePage({Key? key}) : super(key: key);

  @override
  State<TetherHomePage> createState() => _TetherHomePageState();
}

class _TetherHomePageState extends State<TetherHomePage> {
  final IDTextEditingController _remoteId = IDTextEditingController();
  int _nav = 0;
  bool _fileTransfer = false;
  bool _dark = true;

  // ---- theme-aware palette ----
  Color get cBlue => const Color(0xFF2F6BFF);
  Color get cBtnBlue => const Color(0xFF2563EB);
  Color get cGreen => const Color(0xFF25C165);
  Color get cPageBg => _dark ? const Color(0xFF0A0D13) : Colors.white;
  Color get cSidebarBg => _dark ? const Color(0xFF0C1017) : const Color(0xFFF6F7F9);
  Color get cCardBg => _dark ? const Color(0xFF11161F) : Colors.white;
  Color get cInk => _dark ? const Color(0xFFE9EDF4) : const Color(0xFF1F2533);
  Color get cMuted => _dark ? const Color(0xFF8790A1) : const Color(0xFF8B92A2);
  Color get cLine => _dark ? const Color(0xFF1F2734) : const Color(0xFFEDEFF3);
  Color get cActiveNav => _dark ? const Color(0xFF16233E) : const Color(0xFFE9F0FE);
  Color get cInputBg => _dark ? const Color(0xFF0D131C) : Colors.white;

  static const _navIcons = [
    Icons.cast_connected_outlined, // Remote Control (recent preview)
    Icons.dvr_outlined, // Sessions (full recent + search/select/delete)
    Icons.star_outline, // Favorite
    Icons.lan_outlined, // Discovered (LAN)
    Icons.contacts_outlined, // Address Book
    Icons.folder_shared_outlined, // File Transfer
    Icons.settings_outlined, // Settings
  ];
  static const _navLabels = [
    'Remote Control',
    'Sessions',
    'Favorite',
    'Discovered',
    'Address Book',
    'File Transfer',
    'Settings',
  ];
  static const _sessionsNav = 1;
  static const _favoriteNav = 2;
  static const _discoveredNav = 3;
  static const _addressBookNav = 4;
  static const _fileTransferNav = 5;
  static const _settingsNav = 6;
  // Max recent-session cards shown on the home preview.
  static const _homeRecentLimit = 5;

  @override
  void initState() {
    super.initState();
    // Peer cards / connect flow expect a registered IDTextEditingController.
    if (!Get.isRegistered<IDTextEditingController>()) {
      Get.put<IDTextEditingController>(_remoteId);
    }
    // The home opens on Remote Control, so the panel must show recent peers,
    // not whatever tab was persisted from a previous session (e.g. address book).
    WidgetsBinding.instance.addPostFrameCallback((_) {
      gFFI.peerTabModel.setCurrentTab(PeerTabIndex.recent.index);
    });
  }

  @override
  void dispose() {
    if (Get.isRegistered<IDTextEditingController>()) {
      Get.delete<IDTextEditingController>();
    }
    _remoteId.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    _dark = Theme.of(context).brightness == Brightness.dark;
    return Container(
      color: cPageBg,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _sidebar(),
          Expanded(child: _content()),
        ],
      ),
    );
  }

  // ---------- Sidebar ----------
  Widget _sidebar() {
    return Container(
      width: 234,
      color: cSidebarBg,
      padding: const EdgeInsets.fromLTRB(16, 22, 16, 18),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(left: 6, bottom: 26),
            child: Row(
              children: [
                _logo(42),
                const SizedBox(width: 12),
                Text('Tether',
                    style: TextStyle(
                        fontSize: 22, fontWeight: FontWeight.w700, color: cInk)),
              ],
            ),
          ),
          for (int i = 0; i < _navLabels.length; i++) _navItem(i),
          const Spacer(),
          _statusFooter(),
        ],
      ),
    );
  }

  Widget _logo(double size) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(size * 0.28),
      child: Image.asset('assets/icon.png',
          width: size, height: size, fit: BoxFit.cover),
    );
  }

  Widget _navItem(int i) {
    final active = _nav == i;
    return InkWell(
      borderRadius: BorderRadius.circular(10),
      onTap: () {
        if (i == _settingsNav) {
          DesktopTabPage.onAddSetting();
          return;
        }
        setState(() {
          _nav = i;
          _fileTransfer = (i == _fileTransferNav);
        });
        // The Sessions view uses the full peer-tab widget; default it to recent.
        if (i == _sessionsNav) {
          gFFI.peerTabModel.setCurrentTab(PeerTabIndex.recent.index);
        }
      },
      child: Container(
        margin: const EdgeInsets.only(bottom: 6),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
        decoration: BoxDecoration(
          color: active ? cActiveNav : Colors.transparent,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          children: [
            Icon(_navIcons[i], size: 20, color: active ? cBlue : cMuted),
            const SizedBox(width: 12),
            Text(_navLabels[i],
                style: TextStyle(
                    fontSize: 15,
                    fontWeight: active ? FontWeight.w600 : FontWeight.w500,
                    color: active ? cBlue : cInk)),
          ],
        ),
      ),
    );
  }

  Widget _statusFooter() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(children: [
          Container(
              width: 9,
              height: 9,
              decoration: BoxDecoration(color: cGreen, shape: BoxShape.circle)),
          const SizedBox(width: 8),
          Text('Ready',
              style: TextStyle(
                  fontSize: 14, color: cInk, fontWeight: FontWeight.w500)),
        ]),
        const SizedBox(height: 6),
        Row(children: [
          Icon(Icons.lock_outline, size: 15, color: cMuted),
          const SizedBox(width: 8),
          Text('Encrypted connection',
              style: TextStyle(fontSize: 13, color: cMuted)),
        ]),
      ],
    );
  }

  // ---------- Home content ----------
  Widget _home() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 24, 24, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (!bind.mainIsInstalled() && (isWindows || isMacOS)) _installBanner(),
          IntrinsicHeight(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Expanded(child: _thisDeviceCard()),
                const SizedBox(width: 24),
                Expanded(child: _remoteControlCard()),
              ],
            ),
          ),
          const SizedBox(height: 20),
          // Recent Sessions preview (max 5). Fills the remaining height so the
          // window has no bottom blank; the panel is tall enough for all 5 rows.
          Expanded(
            child: _peersCard(
              'Recent Sessions',
              trailing: InkWell(
                onTap: () {
                  gFFI.peerTabModel.setCurrentTab(PeerTabIndex.recent.index);
                  setState(() => _nav = _sessionsNav);
                },
                child: Text('View all',
                    style: TextStyle(
                        fontSize: 14,
                        color: cBlue,
                        fontWeight: FontWeight.w500)),
              ),
              child: RecentPeersView(
                  peerCountLimit: _homeRecentLimit,
                  peerCardUiTypeOverride: PeerUiType.list),
            ),
          ),
          // Real online/connection status.
          Align(alignment: Alignment.centerLeft, child: OnlineStatusWidget()),
        ],
      ),
    );
  }

  // A titled panel (e.g. "Recent Sessions") wrapping a peer view.
  Widget _peersCard(String title, {Widget? trailing, required Widget child}) {
    return Container(
      decoration: BoxDecoration(
        color: cCardBg,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: cLine),
      ),
      clipBehavior: Clip.antiAlias,
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(title,
                  style: TextStyle(
                      fontSize: 17, fontWeight: FontWeight.w700, color: cInk)),
              if (trailing != null) trailing,
            ],
          ),
          const SizedBox(height: 4),
          Expanded(child: child),
        ],
      ),
    );
  }

  Widget _installBanner() {
    return Container(
      margin: const EdgeInsets.only(bottom: 16),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: BoxDecoration(
        color: cBtnBlue.withOpacity(_dark ? 0.16 : 0.10),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: cBtnBlue.withOpacity(0.4)),
      ),
      child: Row(children: [
        Icon(Icons.info_outline, color: cBlue, size: 20),
        const SizedBox(width: 12),
        Expanded(
          child: Text(
            'Install Tether to this system for unattended access and to avoid UAC prompts.',
            style: TextStyle(fontSize: 13.5, color: cInk),
          ),
        ),
        const SizedBox(width: 12),
        ElevatedButton(
          onPressed: () => bind.mainGotoInstall(),
          style: ElevatedButton.styleFrom(
            backgroundColor: cBtnBlue,
            foregroundColor: Colors.white,
            elevation: 0,
            shape:
                RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
          ),
          child: const Text('Install'),
        ),
      ]),
    );
  }

  Widget _card({required Widget child}) {
    return Container(
      padding: const EdgeInsets.all(24),
      decoration: BoxDecoration(
        color: cCardBg,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: cLine),
      ),
      child: child,
    );
  }

  Widget _cardTitle(String t) =>
      Text(t, style: TextStyle(fontSize: 20, fontWeight: FontWeight.w700, color: cBlue));

  Widget _sub(String t) => Padding(
        padding: const EdgeInsets.only(top: 8, bottom: 18),
        child: Text(t, style: TextStyle(fontSize: 13.5, color: cMuted)),
      );

  void _copy(String text) {
    Clipboard.setData(ClipboardData(text: text));
    showToast(translate('Copied'));
  }

  Widget _iconBtn(IconData icon, VoidCallback onTap, {String? tip}) => IconButton(
        splashRadius: 18,
        constraints: const BoxConstraints(),
        padding: const EdgeInsets.all(6),
        tooltip: tip,
        onPressed: onTap,
        icon: Icon(icon, size: 18, color: cMuted),
      );

  void _openSafety() => DesktopSettingPage.switch2page(SettingsTabKey.safety);

  Widget _thisDeviceCard() {
    return ChangeNotifierProvider.value(
      value: gFFI.serverModel,
      child: Consumer<ServerModel>(
        builder: (context, model, _) {
          final id = model.serverId.text;
          final pwd = model.serverPasswd.text;
          final permanent = model.verificationMethod == kUsePermanentPassword;
          final showOneTime = model.approveMode != 'click' && !permanent;
          return _card(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _cardTitle('This Device'),
                _sub('Share this ID and password to allow remote access to this device.'),
                Text('Your ID', style: TextStyle(fontSize: 13, color: cMuted)),
                Row(children: [
                  Expanded(
                    child: Text(id,
                        style: TextStyle(
                            fontSize: 26,
                            fontWeight: FontWeight.w700,
                            color: cBlue)),
                  ),
                  _iconBtn(Icons.copy_outlined, () => _copy(id), tip: translate('Copy')),
                ]),
                Divider(height: 28, color: cLine),
                Text(showOneTime ? 'One-time Password' : translate('Password'),
                    style: TextStyle(fontSize: 13, color: cMuted)),
                Row(children: [
                  Expanded(
                    child: Text(pwd,
                        style: TextStyle(
                            fontSize: 24,
                            fontWeight: FontWeight.w700,
                            color: cBlue)),
                  ),
                  if (showOneTime)
                    _iconBtn(Icons.refresh, () => bind.mainUpdateTemporaryPassword(),
                        tip: translate('Refresh Password')),
                  if (showOneTime)
                    _iconBtn(Icons.copy_outlined, () => _copy(pwd), tip: translate('Copy')),
                  if (!bind.isDisableSettings())
                    _iconBtn(Icons.edit_outlined, _openSafety,
                        tip: translate('Change Password')),
                ]),
                const SizedBox(height: 16),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Row(children: [
                      SizedBox(
                        width: 22,
                        height: 22,
                        child: Checkbox(
                          value: permanent,
                          activeColor: cBtnBlue,
                          onChanged: (_) => _openSafety(),
                        ),
                      ),
                      const SizedBox(width: 10),
                      Text('Unattended access',
                          style: TextStyle(fontSize: 14, color: cInk)),
                    ]),
                    _iconBtn(Icons.settings_outlined,
                        () => DesktopTabPage.onAddSetting(),
                        tip: translate('Settings')),
                  ],
                ),
              ],
            ),
          );
        },
      ),
    );
  }

  Widget _remoteControlCard() {
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _cardTitle('Remote Control'),
          _sub('Enter the remote device ID to connect.'),
          Row(children: [
            Expanded(
              child: TextField(
                controller: _remoteId,
                style: TextStyle(color: cInk, fontSize: 15),
                inputFormatters: [IDTextInputFormatter()],
                onSubmitted: (_) => _doConnect(),
                decoration: InputDecoration(
                  hintText: 'Enter remote ID',
                  hintStyle: TextStyle(color: cMuted),
                  filled: true,
                  fillColor: cInputBg,
                  contentPadding:
                      const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
                  enabledBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(10),
                    borderSide: BorderSide(color: cLine),
                  ),
                  focusedBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(10),
                    borderSide: BorderSide(color: cBlue),
                  ),
                ),
              ),
            ),
            const SizedBox(width: 6),
            _connectTypeMenu(),
          ]),
          const SizedBox(height: 18),
          Row(children: [
            _radio('Remote Control', false),
            const SizedBox(width: 24),
            _radio('File Transfer', true),
          ]),
          const SizedBox(height: 18),
          SizedBox(
            width: 200,
            height: 46,
            child: ElevatedButton(
              onPressed: _doConnect,
              style: ElevatedButton.styleFrom(
                backgroundColor: cBtnBlue,
                foregroundColor: Colors.white,
                elevation: 0,
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(10)),
              ),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: const [
                  Text('Connect',
                      style: TextStyle(
                          fontSize: 15.5, fontWeight: FontWeight.w600)),
                  SizedBox(width: 8),
                  Icon(Icons.arrow_forward, size: 18),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  // More connection types (parity with old home): view camera, terminal,
  // TCP tunneling, RDP.
  Widget _connectTypeMenu() {
    return PopupMenuButton<String>(
      tooltip: translate('Connection Type'),
      icon: Icon(Icons.expand_more, color: cMuted),
      onSelected: (v) {
        final id = _remoteId.id;
        if (id.isEmpty) return;
        switch (v) {
          case 'view_camera':
            connect(context, id, isViewCamera: true);
            break;
          case 'terminal':
            connect(context, id, isTerminal: true);
            break;
          case 'tcp':
            connect(context, id, isTcpTunneling: true);
            break;
          case 'rdp':
            connect(context, id, isRDP: true);
            break;
          case 'file':
            connect(context, id, isFileTransfer: true);
            break;
        }
      },
      itemBuilder: (_) => [
        PopupMenuItem(value: 'file', child: Text(translate('Transfer File'))),
        PopupMenuItem(value: 'view_camera', child: Text(translate('View camera'))),
        PopupMenuItem(value: 'terminal', child: Text(translate('Terminal'))),
        PopupMenuItem(value: 'tcp', child: Text(translate('TCP Tunneling'))),
        PopupMenuItem(value: 'rdp', child: Text('RDP')),
      ],
    );
  }

  Widget _radio(String label, bool value) {
    return InkWell(
      onTap: () => setState(() => _fileTransfer = value),
      child: Row(children: [
        Radio<bool>(
          value: value,
          groupValue: _fileTransfer,
          activeColor: cBlue,
          materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
          onChanged: (v) => setState(() => _fileTransfer = v ?? false),
        ),
        const SizedBox(width: 4),
        Text(label, style: TextStyle(fontSize: 14, color: cInk)),
      ]),
    );
  }

  void _doConnect() {
    final id = _remoteId.id;
    if (id.isEmpty) return;
    connect(context, id, isFileTransfer: _fileTransfer);
  }

  // Route the selected sidebar section to its content.
  Widget _content() {
    switch (_nav) {
      case _sessionsNav:
        return _sessionsView();
      case _favoriteNav:
        return _sectionView('Favorite', FavoritePeersView());
      case _discoveredNav:
        return _sectionView('Discovered', DiscoveredPeersView());
      case _addressBookNav:
        return _sectionView('Address Book', AddressBook());
      default:
        return _home(); // Remote Control (0) and File Transfer (5)
    }
  }

  // A full-height section with a single peer view (no top ID/connect cards).
  Widget _sectionView(String title, Widget child) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 24, 24, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(child: _peersCard(title, child: child)),
          Align(alignment: Alignment.centerLeft, child: OnlineStatusWidget()),
        ],
      ),
    );
  }

  // Sessions: full peer manager WITH the toolbar (search / multi-select / delete).
  Widget _sessionsView() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 24, 24, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: Container(
              decoration: BoxDecoration(
                color: cCardBg,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: cLine),
              ),
              clipBehavior: Clip.antiAlias,
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              // full toolbar (search / select / delete), but no tab-icon strip
              // — those tabs live in the Tether sidebar.
              child: PeerTabPage(hideTabBar: true),
            ),
          ),
          Align(alignment: Alignment.centerLeft, child: OnlineStatusWidget()),
        ],
      ),
    );
  }
}
