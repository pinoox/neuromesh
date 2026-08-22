export interface Product {
  id: string;
  title: string;
  price: number;
  originalPrice?: number;
  image: string;
  category: string;
  description: string;
  badge?: string;
  rating: number;
}

export interface CartItem {
  product: Product;
  quantity: number;
}
