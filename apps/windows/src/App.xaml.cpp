#include "pch.h"
#include "App.xaml.h"
#include "MainWindow.xaml.h"

#include <shellapi.h>

using namespace winrt;
using namespace winrt::Windows::ApplicationModel::Activation;
using namespace winrt::Windows::Storage;
using namespace winrt::Microsoft::UI::Xaml;
using namespace winrt::Microsoft::Windows::AppLifecycle;

namespace winrt::VexioWindows::implementation {

App::App() {
    InitializeComponent();

    UnhandledException([](IInspectable const&, Microsoft::UI::Xaml::UnhandledExceptionEventArgs const& e) {
        // Keep the workstation alive on recoverable UI layer exceptions while preserving diagnostics.
        e.Handled(true);
    });
}

void App::OnLaunched(Microsoft::UI::Xaml::LaunchActivatedEventArgs const& args) {
    auto mainWindow = make<MainWindow>();
    m_window = mainWindow;
    m_window.Activate();

    auto windowImpl = mainWindow.as<MainWindow>();

    // 1. Handle MSIX File Type Association activation (.vsdx, .vsdm, .vssx, .vssm, .vstx, .vstm, .vsd, .vdx, .dwg, .dxf)
    const AppActivationArguments lifecycleArgs = AppInstance::GetCurrent().GetActivatedEventArgs();
    if (lifecycleArgs && lifecycleArgs.Kind() == ExtendedActivationKind::File) {
        if (auto fileArgs = lifecycleArgs.Data().try_as<IFileActivatedEventArgs>()) {
            const auto files = fileArgs.Files();
            if (files && files.Size() > 0) {
                if (auto firstFile = files.GetAt(0).try_as<StorageFile>()) {
                    windowImpl->OpenStorageFileAsync(firstFile);
                    return;
                }
            }
        }
    }

    // 2. Fallback: Handle Win32 command-line file argument (e.g. `Vexio.exe diagram.vsdx`)
    const hstring cmdLine = args.Arguments();
    if (!cmdLine.empty()) {
        int argc = 0;
        LPWSTR* argv = ::CommandLineToArgvW(cmdLine.c_str(), &argc);
        if (argv != nullptr) {
            const std::wstring firstArg{argc > 0 ? argv[0] : L""};
            ::LocalFree(argv);
            if (!firstArg.empty() && std::filesystem::exists(firstArg)) {
                windowImpl->OpenFileFromPathAsync(hstring{firstArg});
            }
        }
    }
}

} // namespace winrt::VexioWindows::implementation
