import { useEffect, useState, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { usePrivy, useWallets } from "@privy-io/react-auth";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Spinner } from "@/components/ui/spinner";


interface Message {
  role: "user" | "ai";
  title: string;
  content: string;
  timestamp: Date;
  sessionId?: string;
}

interface Transaction {
  hash: string;
  status: "pending" | "completed" | "failed";
  type: string;
  timestamp: Date;
}

type ForgeStep = {
  title: string;
  output: string;
};

interface TransactionDetails {
  to: string;
  function: string;
  arguments: string[];
  value: string;
  input_data: string;
}

interface FixResponse {
  code: string;
  message: string;
}

interface ForgeResponse {
  title: string;
  output: string;
}

const useEventSourceWithRetry = (
  url: string,
  options: {
    onMessage: (event: MessageEvent) => void;
    onError: (error: Event) => void;
    maxRetries?: number;
  }
) => {
  useEffect(() => {
    let retryCount = 0;
    let eventSource: EventSource;

    const connect = () => {
      eventSource = new EventSource(url);
      eventSource.onmessage = options.onMessage;
      eventSource.onerror = (error) => {
        console.error("EventSource failed:", error);
        eventSource.close();

        if (!options.maxRetries || retryCount < options.maxRetries) {
          console.log(`Retrying connection... (${retryCount + 1})`);
          retryCount++;
          setTimeout(connect, 1000 * retryCount); // Exponential backoff
        } else {
          options.onError(error);
        }
      };
    };

    connect();

    return () => {
      eventSource?.close();
    };
  }, [url]);
};

function App() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [currentAccordion, setCurrentAccordion] = useState("0");
  const [input, setInput] = useState("");
  const [transactions, setTransactions] = useState<TransactionDetails[]>([]);
  const [txCount, setTxCount] = useState(0);
  
  const [prompt, setPrompt] = useState("");

  const [tempDir, setTempDir] = useState<string | null>(null);

  const { ready, authenticated, user, login, logout } = usePrivy();

  const messagesEndRef = useRef<HTMLDivElement>(null);

  const {wallets} = useWallets();

  const eventSourceRef = useRef<EventSource | null>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(() => {
    if (!authenticated) {
      login();
    }
  }, []);

  const query = async () => {
    if (!prompt.trim() || !user?.wallet?.address) return;

    const userMessage: Message = {
      role: "user",
      title: "Prompt",
      content: prompt,
      timestamp: new Date(),
    };
    setMessages(prev => [...prev, userMessage]);
    setPrompt("");

    // Close any existing connection
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
    }

    const url = `http://127.0.0.1:3000/forge/stream?${new URLSearchParams({
      intent: prompt,
      from_address: user.wallet.address,
      rpc_url: "http://ethereumreth:8545",
    })}`;

    console.log("Connecting to:", url); // Debug log

    const eventSource = new EventSource(url, {
      withCredentials: false
    });

    // Store reference to close later if needed
    eventSourceRef.current = eventSource;

    eventSource.addEventListener('open', () => {
      console.log('SSE connection opened');
    });

    eventSource.addEventListener('message', (event) => {
      console.log('Received message:', event.data); // Debug log
      const data = JSON.parse(event.data) as ForgeResponse;
      
      if (data.title === "Debug") {
        console.log(data.output);
        return;
      }

      // Store session ID when received
      if (data.title === "Session") {
        setTempDir(data.output);
        console.log(data.output);
        return;
      }

      setMessages((prev) => {
        const messages = [...prev];
        const lastMessage = messages[messages.length - 1];

        if (lastMessage?.content === data.output && lastMessage?.title === data.title) {
          return messages;
        }

        const isNewStep = lastMessage?.title !== data.title && data.title !== "Error";

        if (isNewStep) {
          messages.push({
            role: "ai",
            title: data.title,
            content: data.output,
            timestamp: new Date(),
          });
        } else {
          lastMessage.content += data.output;
          if (data.title === "Error") {
            lastMessage.title += " (Failed)";
            eventSource.close();  // Close the EventSource after receiving the session
          }
        }
        return messages;
      });
    });

    eventSource.addEventListener('close', () => {
      console.log('Stream completed normally');
      eventSource.close();
    });

    eventSource.addEventListener('error', (error) => {
      // Only handle as error if it's not a normal close
      if (!error.eventPhase) {
        console.error('SSE Error:', error);
        eventSource.close();
        setMessages(prev => [...prev, {
          role: 'ai',
          title: 'Connection Error',
          content: 'Lost connection to server. Please try again.',
          timestamp: new Date()
        }]);
      }
    });
  };

  const getFix = async (errorMessage: string) => {
    const lastMessage = messages[messages.length - 1];
    if (!lastMessage?.title.includes("Failed")) return;

    // Use EventSource instead of fetch
    const eventSource = new EventSource(
      `http://127.0.0.1:3000/forge/fix?${new URLSearchParams({
        error: errorMessage,
        rpc_url: "http://ethereumreth:8545",
        temp_dir: tempDir || "",
      })}`
    );

    eventSource.onmessage = (event) => {
      const data = JSON.parse(event.data) as ForgeResponse;

      if (data.title === "Debug") {
        console.log(data.output);
        return;
      }

      if (data.title === "Simulating Transactions") {
        if (data.output.includes("[{")) {
          const transactions = JSON.parse(data.output);
          setTransactions(transactions);
        }
      }
      
      setMessages((prev) => {
        const messages = [...prev];
        const lastMessage = messages[messages.length - 1];

        if (lastMessage?.content === data.output && lastMessage?.title === data.title) {
          return messages;
        }

        const isNewStep = lastMessage?.title !== data.title && data.title !== "Error";

        if (isNewStep) {
          messages.push({
            role: "ai",
            title: data.title,
            content: data.output,
            timestamp: new Date(),
          });
        } else {
          lastMessage.content += data.output;
          if (data.title === "Error") {
            lastMessage.title += " (Failed)";
            eventSource.close();  // Close the EventSource after receiving the session
          }
        }
        return messages;
      });
    };

    eventSource.onerror = (error) => {
      console.error("EventSource failed:", error);
      eventSource.close();
    };
  };

  useEffect(() => {
    console.log(messages);
  }, [messages]);

  // Update currentAccordion when messages change
  useEffect(() => {
    if (messages.length > 0) {
      setCurrentAccordion(String(messages.length - 1));
    }
    scrollToBottom();
  }, [messages]);


  useEffect(() => {
    const handleTransaction = async (transaction: TransactionDetails) => {
      // Add transaction message
      setMessages(prev => [...prev, {
        role: "ai",
        title: transaction.function + " " + txCount +  "/" + transactions.length,
        content: `Please sign this transaction:\n\nTo: ${transaction.to}\nFunction: ${transaction.function}\nArguments: ${transaction.arguments.join(", ")}\nValue: ${transaction.value} ETH`,
        timestamp: new Date(),
      }]);

      try {
        const wallet = wallets[0];

        const provider = await wallet.getEthereumProvider();
  
        const result = await provider.request({
          method: "eth_sendTransaction",
          params: [
            {
              from: user?.wallet?.address,
              to: transaction.to,
              data: transaction.input_data,
              value: transaction.value,
            },
          ],
        });
        
        // Add success message
        setMessages(prev => [...prev, {
          role: "ai",
          title: "Transaction",
          content: `Transaction sent! Hash: ${result.hash}`,
          timestamp: new Date(),
        }]);

        // Remove this transaction and process next one
        setTransactions(prev => prev.slice(1));
        setTxCount(prev => prev + 1);
      } catch (e: any) {
        // Add error message
        setMessages(prev => {
          const newMessages = [...prev];
          const lastMessage = newMessages[newMessages.length - 1];
          if (lastMessage?.title === "Transaction") {
            lastMessage.title += " (Failed)";
          }
          lastMessage.content += "\n" + e.message || "Transaction failed \n";
          return newMessages;
        });
      }
    };

    if (transactions.length > 0) {
      handleTransaction(transactions[0]);
    }

  }, [transactions, user?.wallet]);

  return (
    <div className="min-h-screen max-w-3xl flex flex-col">
      {/* Fixed Navbar */}
      <div className="fixed top-0 left-0 right-0 bg-[#0a0a0a] z-10">
        <div className="max-w-3xl mx-auto p-4">
          <div className="flex justify-end">
            {authenticated ? (
              <div className="flex items-center gap-3  px-4 py-2 rounded-lg ">
                <span className="text-zinc-300 font-semibold text-sm max-w-[200px]">
                  {user?.wallet?.address.slice(0, 6)}...
                  {user?.wallet?.address.slice(-6)}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => logout()}
                  className="text-zinc-400 hover:text-white hover:focus-none  ring-0 hover:ring-0"
                >
                  Logout
                </Button>
              </div>
            ) : (
              <Button onClick={() => login()}>Connect Wallet</Button>
            )}
          </div>
        </div>
      </div>

      {/* Messages - add padding for navbar and search bar */}
      <div className="flex-1 overflow-auto w-full p-4 pt-20 pb-32">
        <div className="max-w-3xl w-full flex flex-col gap-2">
          <Accordion
            type="single"
            value={currentAccordion}
            onValueChange={setCurrentAccordion}
            collapsible
          >
            {messages.map((message, index) =>
              message.role === "user" ? (
                <p className="lg:px-5 p-2 text-gray-200 whitespace-pre-wrap">
                  {message.content}
                </p>
              ) : (
                <AccordionItem
                  key={index}
                  value={String(index)}
                  className="border-0"
                >
                  <AccordionTrigger className={`hover:no-underline flex items-center gap-2 ${
                    index === messages.length - 1 ? "" : " text-xs text-[hsl(var(--muted-foreground))]"
                  }`}>
                    <div className="w-full flex items-center justify-between gap-2">
                      <span className={`${index === messages.length - 1 ? "" : "text-sm text-[hsl(var(--muted-foreground))]" }`}>
                        {message.title}
                      </span>
                      { message.title.includes("Failed") ? (
                        <svg className="h-4 w-4 text-red-500" viewBox="0 0 20 20" fill="currentColor">
                          <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clipRule="evenodd" />
                        </svg>
                      ) : index === messages.length - 1 ? (
                        <Spinner />
                      ) : (
                        <svg className="h-4 w-4 text-green-500" viewBox="0 0 20 20" fill="currentColor">
                          <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                        </svg>
                      )}
                    </div>
                  </AccordionTrigger>
                  <AccordionContent
                    className="lg:px-5"
                  >
                    <pre className="text-sm text-gray-200 whitespace-pre-wrap font-mono bg-black/30 lg:p-4 rounded-lg overflow-x-auto">
                      {message.content}
                    </pre>
                    {message.title.includes("Failed") && (
                      <div className="mt-2 flex justify-end">
                        <Button 
                          variant="default" 
                          size="sm"
                          className="bg-blue-400 hover:bg-blue-500 hover:text-white"
                          onClick={() => getFix(message.content)}
                        >
                          Get Fix
                        </Button>
                      </div>
                    )}
                  </AccordionContent>
                </AccordionItem>
              )
            )}
          </Accordion>
          <div ref={messagesEndRef} /> {/* Scroll anchor */}
        </div>
      </div>

      {/* Fixed Search Bar */}
      <div className="fixed bottom-0 left-0 right-0 bg-[#0a0a0a] z-10">
        <div className="max-w-3xl mx-auto">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (prompt) {
                query();
              }
            }}
          >
            <label
              className="flex items-center justify-center border bg-[#101010] px-4 pt-5 pb-8 rounded-t-lg gap-2 border-[#252525] shadow-[#0d0d0d] shadow-xl"
              htmlFor="search-bar"
            >
              <input
                id="search-bar"
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                autoFocus
                placeholder="Ask anything..."
                className="px-2 pr-6 w-full flex-1 outline-none bg-inherit appearance-none text-white"
              />
              <button
                type="submit"
                className="w-auto h-full text-white active:scale-95 overflow-hidden relative rounded-lg"
              >
                <img src="/icons/search.svg" alt="search" className="h-4 w-4" />
              </button>
            </label>
          </form>
        </div>
      </div>
    </div>
  );
}

export default App;
