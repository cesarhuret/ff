import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { PrivyProvider } from "@privy-io/react-auth";
import {mainnet} from 'viem/chains';

createRoot(document.getElementById('root')!).render(
     <PrivyProvider
      appId="clzhp6hio00yjd3qba4uhh4ho"
      config={{
        defaultChain: mainnet,
        // Display email and wallet as login methods
        loginMethods: ["email", "wallet", "farcaster", "telegram"],
        // Customize Privy's appearance in your app
        appearance: {
          theme: "#000",
          accentColor: "#000",
          logo: "https://pub-dc971f65d0aa41d18c1839f8ab426dcb.r2.dev/privy.png",
          walletList: [
            "coinbase_wallet",
            "metamask",
            "rainbow",
            "rabby_wallet",
          ],
        },
        // Create embedded wallets for users who don't have a wallet
        embeddedWallets: {
          createOnLogin: "users-without-wallets",
        },
        externalWallets: {
          coinbaseWallet: {
            // Valid connection options include 'eoaOnly' (default), 'smartWalletOnly', or 'all'
            connectionOptions: "smartWalletOnly",
          },
        },
      }}
    >
      <App />
    </PrivyProvider>
  ,
)
