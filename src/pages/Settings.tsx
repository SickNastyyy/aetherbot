import { motion } from 'framer-motion'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { WalletCredForm } from '@/components/settings/WalletCredForm'
import { KalshiConfig } from '@/components/settings/KalshiConfig'
import { RiskParams } from '@/components/settings/RiskParams'

export function Settings() {
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.3 }}
      className="max-w-3xl mx-auto p-5"
    >
      <div className="mb-5">
        <h2 className="text-[15px] font-semibold text-holo-text-bright">Settings</h2>
        <p className="text-[12px] text-holo-text-muted mt-0.5">
          Configure DEX wallets, prediction market API, and risk parameters
        </p>
      </div>

      <Tabs defaultValue="wallets" className="space-y-3">
        <TabsList className="bg-holo-surface border border-holo-border h-9">
          <TabsTrigger
            value="wallets"
            className="text-[12px] data-[state=active]:bg-holo-surface-2 data-[state=active]:text-holo-text-bright"
          >
            DEX Wallets
          </TabsTrigger>
          <TabsTrigger
            value="kalshi"
            className="text-[12px] data-[state=active]:bg-holo-surface-2 data-[state=active]:text-holo-text-bright"
          >
            Kalshi
          </TabsTrigger>
          <TabsTrigger
            value="risk"
            className="text-[12px] data-[state=active]:bg-holo-surface-2 data-[state=active]:text-holo-text-bright"
          >
            Risk & Mode
          </TabsTrigger>
        </TabsList>

        <TabsContent value="wallets" className="panel p-5">
          <div className="grid grid-cols-2 gap-6">
            <WalletCredForm
              exchange="Jupiter"
              chain="Solana"
              placeholder="Base58 private key..."
              description="Jupiter is the leading Solana DEX aggregator. Provide your Solana wallet private key to sign swap transactions. Supports SOL, JUP, BONK, and all SPL tokens."
            />
            <WalletCredForm
              exchange="Uniswap"
              chain="Ethereum"
              placeholder="0x..."
              description="Uniswap is the top EVM DEX. Provide your Ethereum wallet private key for swaps on Ethereum, Arbitrum, Base, and other L2s. Supports ETH, WBTC, and all ERC-20 tokens."
            />
          </div>
          <div className="border-t border-holo-border mt-5 pt-3">
            <p className="text-[11px] text-holo-text-muted leading-relaxed">
              In desktop mode, wallet keys are encrypted with AES-256-GCM and stored locally. They never leave your machine.
              In browser dev mode, keys are only held in memory and lost on reload.
            </p>
          </div>
        </TabsContent>

        <TabsContent value="kalshi" className="panel p-5">
          <KalshiConfig />
        </TabsContent>

        <TabsContent value="risk" className="panel p-5">
          <RiskParams />
        </TabsContent>
      </Tabs>
    </motion.div>
  )
}
