#pragma once

#include "pch.h"
#include "App.xaml.g.h"

namespace winrt::VexioWindows::implementation {

struct App : AppT<App> {
    App();

    void OnLaunched(winrt::Microsoft::UI::Xaml::LaunchActivatedEventArgs const& args);

private:
    winrt::Microsoft::UI::Xaml::Window m_window{nullptr};
};

} // namespace winrt::VexioWindows::implementation
