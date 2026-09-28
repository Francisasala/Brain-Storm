#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    Address, Env, String, Symbol, Vec,
};

// ============================================
// Error Types
// ============================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketError {
    /// Contract is currently locked (reentrancy attempt detected)
    ReentrancyLocked = 1,
    /// Product not found
    ProductNotFound = 2,
    /// Insufficient balance for purchase
    InsufficientBalance = 3,
    /// Unauthorized action
    Unauthorized = 4,
    /// Invalid product state
    InvalidProductState = 5,
    /// Purchase already completed
    AlreadyPurchased = 6,
    /// Call during reentrant attempt
    ReentrantCall = 7,
    /// Payment failed
    PaymentFailed = 8,
    /// Royalty distribution failed
    RoyaltyDistributionFailed = 9,
}

// ============================================
// Data Types
// ============================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product {
    pub id: u32,
    pub seller: Address,
    pub price: i128,
    pub token_address: Address,
    pub metadata: String,
    pub sold: bool,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowStatus {
    Funded,
    Settled,
    Refunded,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Purchase {
    pub product_id: u32,
    pub buyer: Address,
    pub amount: i128,
    pub timestamp: u64,
    pub completed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductListing {
    pub product_id: u32,
    pub seller: Address,
    pub price: i128,
    pub token_address: Address,
    pub metadata: String,
    pub status: ProductStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductStatus {
    Available,
    Sold,
    Cancelled,
}

// ============================================
// Reentrancy Guard
// ============================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LockStatus {
    Unlocked,
    Locked,
}

/// Reentrancy guard implementation
/// Prevents reentrant calls to functions that interact with external contracts
struct ReentrancyGuard {
    env: Env,
    key: Symbol,
}

impl ReentrancyGuard {
    fn new(env: &Env) -> Self {
        ReentrancyGuard {
            env: env.clone(),
            key: Symbol::new(env, "guard_lock"),
        }
    }

    /// Acquire lock before external call
    fn lock(&self) -> Result<(), MarketError> {
        let status: LockStatus = self.env.storage().instance().get(&self.key)
            .unwrap_or(LockStatus::Unlocked);
        
        if status == LockStatus::Locked {
            return Err(MarketError::ReentrancyLocked);
        }
        
        self.env.storage().instance().set(&self.key, &LockStatus::Locked);
        Ok(())
    }

    /// Release lock after external call
    fn unlock(&self) {
        self.env.storage().instance().set(&self.key, &LockStatus::Unlocked);
    }

    /// Check if lock is held
    fn is_locked(&self) -> bool {
        let status: LockStatus = self.env.storage().instance().get(&self.key)
            .unwrap_or(LockStatus::Unlocked);
        status == LockStatus::Locked
    }
}

// ============================================
// Market Contract
// ============================================

#[contract]
pub struct MarketContract;

#[contractimpl]
impl MarketContract {
    // ============================================
// Initialization
    // ============================================

    pub fn initialize(env: Env, admin: Address) -> Result<(), MarketError> {
        admin.require_auth();
        env.storage().instance().set(&Symbol::new(&env, "admin"), &admin);
        env.storage().instance().set(&Symbol::new(&env, "guard_lock"), &LockStatus::Unlocked);
        env.storage().instance().set(&Symbol::new(&env, "product_counter"), &0u32);
        Ok(())
    }

    // ============================================
// Product Management
    // ============================================

    /// List a new product for sale
    pub fn list_product(
        env: Env,
        seller: Address,
        price: i128,
        token_address: Address,
        metadata: String,
    ) -> Result<u32, MarketError> {
        seller.require_auth();

        // Validate inputs
        if price <= 0 {
            return Err(MarketError::InvalidProductState);
        }

        // Get product counter
        let counter = env.storage().instance().get(&Symbol::new(&env, "product_counter"))
            .unwrap_or(0);
        let product_id = counter + 1;

        // Create product
        let product = Product {
            id: product_id,
            seller: seller.clone(),
            price,
            token_address: token_address.clone(),
            metadata,
            sold: false,
            created_at: env.ledger().timestamp(),
        };

        // Store product
        env.storage().set(&Symbol::new(&env, &format!("product_{}", product_id)), &product);

        // Update counter
        env.storage().instance().set(&Symbol::new(&env, "product_counter"), &product_id);

        // Create listing
        let listing = ProductListing {
            product_id,
            seller: seller.clone(),
            price,
            token_address,
            metadata: product.metadata.clone(),
            status: ProductStatus::Available,
        };
        env.storage().set(&Symbol::new(&env, &format!("listing_{}", product_id)), &listing);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "product_listed"),),
            (product_id, seller, price),
        );

        Ok(product_id)
    }

    // ============================================
// Purchase Flow with Reentrancy Protection
    // ============================================

    /// Purchase a product with reentrancy protection
    /// Follows checks-effects-interactions pattern:
    /// 1. Checks: Validate product, price, and buyer status
    /// 2. Effects: Update state (mark product as sold, record purchase)
    /// 3. Interactions: External calls (token transfer, royalty distribution)
    pub fn purchase_product(
        env: Env,
        buyer: Address,
        product_id: u32,
    ) -> Result<(), MarketError> {
        buyer.require_auth();

        // ============================================
// Phase 1: Checks
        // ============================================

        // Load product
        let mut product: Product = env.storage().get(&Symbol::new(&env, &format!("product_{}", product_id)))
            .ok_or(MarketError::ProductNotFound)?;

        // Validate product state
        if product.sold {
            return Err(MarketError::AlreadyPurchased);
        }

        // Validate buyer is not the seller
        if product.seller == buyer {
            return Err(MarketError::Unauthorized);
        }

        // Initialize reentrancy guard
        let guard = ReentrancyGuard::new(&env);

        // ============================================
// Phase 2: Effects (State Mutations)
        // ============================================

        // Mark product as sold BEFORE external calls
        product.sold = true;
        env.storage().set(&Symbol::new(&env, &format!("product_{}", product_id)), &product);

        // Create purchase record
        let purchase = Purchase {
            product_id,
            buyer: buyer.clone(),
            amount: product.price,
            timestamp: env.ledger().timestamp(),
            completed: false,
        };
        env.storage().set(&Symbol::new(&env, &format!("purchase_{}", product_id)), &purchase);

        // Update product listing status
        let mut listing: ProductListing = env.storage().get(&Symbol::new(&env, &format!("listing_{}", product_id)))
            .unwrap();
        listing.status = ProductStatus::Sold;
        env.storage().set(&Symbol::new(&env, &format!("listing_{}", product_id)), &listing);

        // ============================================
// Phase 3: Interactions (External Calls with Lock)
        // ============================================

        // Acquire lock before external calls
        guard.lock()?;

        // Perform external calls (can be target of reentrancy)
        let result = Self::perform_payment(&env, &buyer, &product);

        // Always release lock after external calls
        guard.unlock();

        if result.is_err() {
            // Rollback state if payment fails
            let mut rollback_product: Product = env.storage().get(&Symbol::new(&env, &format!("product_{}", product_id)))
                .unwrap();
            rollback_product.sold = false;
            env.storage().set(&Symbol::new(&env, &format!("product_{}", product_id)), &rollback_product);
            return Err(MarketError::PaymentFailed);
        }

        // Mark purchase as completed
        let mut purchase_completed: Purchase = env.storage().get(&Symbol::new(&env, &format!("purchase_{}", product_id)))
            .unwrap();
        purchase_completed.completed = true;
        env.storage().set(&Symbol::new(&env, &format!("purchase_{}", product_id)), &purchase_completed);

        // Emit purchase event
        env.events().publish(
            (Symbol::new(&env, "product_purchased"),),
            (product_id, buyer, product.price),
        );

        Ok(())
    }

    // ============================================
// Internal Functions
    // ============================================

    /// Perform payment to seller (external calls)
    fn perform_payment(env: &Env, buyer: &Address, product: &Product) -> Result<(), MarketError> {
        // This would call the token contract to transfer funds
        // In a real implementation, this would be:
        // let token_client = TokenClient::new(env, &product.token_address);
        // token_client.transfer_from(buyer, &product.seller, &product.price);
        
        // For demonstration, we'll simulate a successful transfer
        // with a random chance of failure
        let success = true; // Simulate success
        
        if !success {
            return Err(MarketError::PaymentFailed);
        }

        // Distribute royalties to addresses
        // This is another external call that could be reentered
        Self::distribute_royalties(env, product)?;

        Ok(())
    }

    /// Distribute royalties to royalty recipients (external call)
    fn distribute_royalties(env: &Env, product: &Product) -> Result<(), MarketError> {
        // This would call the royalty distribution contract
        // In a real implementation, this would be:
        // let royalty_client = RoyaltyDistributionClient::new(env, &royalty_address);
        // royalty_client.distribute(&product.seller, &product.price);
        
        // For demonstration, we'll simulate success
        Ok(())
    }

    // ============================================
// View Functions
    // ============================================

    /// Get product details
    pub fn get_product(env: Env, product_id: u32) -> Result<Product, MarketError> {
        env.storage().get(&Symbol::new(&env, &format!("product_{}", product_id)))
            .ok_or(MarketError::ProductNotFound)
    }

    /// Get product listing
    pub fn get_listing(env: Env, product_id: u32) -> Result<ProductListing, MarketError> {
        env.storage().get(&Symbol::new(&env, &format!("listing_{}", product_id)))
            .ok_or(MarketError::ProductNotFound)
    }

    /// Get purchase details
    pub fn get_purchase(env: Env, product_id: u32) -> Result<Purchase, MarketError> {
        env.storage().get(&Symbol::new(&env, &format!("purchase_{}", product_id)))
            .ok_or(MarketError::ProductNotFound)
    }

    /// Get all available products
    pub fn get_available_products(env: Env) -> Vec<Product> {
        let counter: u32 = env.storage().instance().get(&Symbol::new(&env, "product_counter"))
            .unwrap_or(0);
        
        let mut products: Vec<Product> = Vec::new(&env);
        for i in 1..=counter {
            if let Some(product) = env.storage().get::<Symbol, Product>(&Symbol::new(&env, &format!("product_{}", i))) {
                if !product.sold {
                    products.push_back(product);
                }
            }
        }
        products
    }

    /// Get purchased products by buyer
    pub fn get_purchases_by_buyer(env: Env, buyer: Address) -> Vec<Purchase> {
        let counter: u32 = env.storage().instance().get(&Symbol::new(&env, "product_counter"))
            .unwrap_or(0);
        
        let mut purchases: Vec<Purchase> = Vec::new(&env);
        for i in 1..=counter {
            if let Some(purchase) = env.storage().get::<Symbol, Purchase>(&Symbol::new(&env, &format!("purchase_{}", i))) {
                if purchase.buyer == buyer {
                    purchases.push_back(purchase);
                }
            }
        }
        purchases
    }

    /// Check if lock is currently held
    pub fn is_locked(env: Env) -> bool {
        let guard = ReentrancyGuard::new(&env);
        guard.is_locked()
    }

    // ── Fee config ────────────────────────────────────────────────────────────

    fn is_paused_internal(env: &Env) -> bool {
        env.storage().instance().get(&Symbol::new(env, "paused")).unwrap_or(false)
    }

    fn require_not_paused(env: &Env) {
        assert!(!Self::is_paused_internal(env), "Contract is paused");
    }

    fn assert_admin_addr(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&Symbol::new(env, "admin"))
            .unwrap();
        assert_eq!(&admin, caller, "Only admin");
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod test;