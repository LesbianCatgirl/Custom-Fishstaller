using Bloxstrap.UI.ViewModels.Settings;

namespace Bloxstrap.UI.Elements.Settings.Pages
{
    public partial class BanAsyncPage
    {
        public BanAsyncPage()
        {
            DataContext = new BanAsyncViewModel();
            InitializeComponent();
        }
    }
}
